//! `clios session`: reabre as janelas da última sessão.
//!
//! Num desktop onde quase tudo é um terminal, isso é possível de um jeito que um desktop gráfico não consegue: cada
//! terminal volta na pasta em que estava, com o editor ou o app que estava aberto, na mesma workspace. As TUIs do
//! catálogo voltam pelo id, e o navegador volta uma vez (ele restaura as próprias abas).
//!
//! O que nunca é repetido: comandos que não estão na lista de programas interativos (um `rm` ou um `cargo build` não
//! voltam a rodar sozinhos) e qualquer linha com pipe, redirecionamento ou substituição.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use super::term::{self, Card, Launch};
use crate::catalog::Catalog;
use crate::ctx::Ctx;
use crate::hub::{exec, items::Action};
use crate::sys::is_installed;
use crate::ui;

/// Programas interativos que podem voltar a rodar sozinhos, além dos binários do catálogo.
const SAFE: &[&str] = &["hx", "helix", "nvim", "vim", "vi", "nano", "less", "man", "glow", "tig", "lnav", "ncdu"];

/// Classes que nunca entram: o hub, a proteção de tela, o seletor de arquivos e o scratchpad (que nasce sozinho).
const SKIP: &[&str] = &["clios.hub", "clios.saver", "clios.scratch", "clios.float.picker"];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum What {
    /// Um terminal: a pasta e, se for seguro, o comando que estava rodando.
    Term {
        cwd: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        run: Option<String>,
        #[serde(default)]
        float: bool,
    },
    /// Uma entrada do catálogo (`clios open <id>`).
    App { id: String },
    /// Um programa gráfico que se restaura sozinho (o navegador).
    Gui { cmd: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Win {
    pub workspace: i64,
    #[serde(flatten)]
    pub what: What,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Session {
    /// Quando foi salva (segundos desde 1970).
    pub saved: i64,
    pub windows: Vec<Win>,
}

/// Uma janela do `hyprctl clients -j`, só com o que interessa.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct Client {
    #[serde(default)]
    pub class: String,
    #[serde(default, rename = "initialTitle")]
    pub initial_title: String,
    #[serde(default)]
    pub workspace: ClientWs,
    #[serde(default)]
    pub at: (i64, i64),
    #[serde(default = "yes")]
    pub mapped: bool,
}

fn yes() -> bool {
    true
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct ClientWs {
    #[serde(default)]
    pub id: i64,
}

/// O comando volta a rodar? Só se for um programa interativo conhecido e a linha for simples.
pub fn safe_run(cmd: &str, catalog_bins: &[&str]) -> Option<String> {
    let cmd = cmd.trim();
    if cmd.is_empty() || cmd.contains([';', '|', '&', '<', '>', '`', '$', '\n', '\\']) {
        return None;
    }
    let first = cmd.split_whitespace().next()?;
    (SAFE.contains(&first) || catalog_bins.contains(&first)).then(|| cmd.to_string())
}

/// Monta a sessão a partir das janelas abertas e das fichas dos terminais. Pura: os testes dão os dois lados.
pub fn capture(clients: &[Client], card: &dyn Fn(&str) -> Option<Card>, catalog: &Catalog, now: i64) -> Session {
    let bins: Vec<&str> = catalog.tui.iter().map(|t| t.bin()).collect();
    let mut sorted: Vec<&Client> = clients.iter().filter(|c| c.mapped && c.workspace.id > 0).collect();
    sorted.sort_by_key(|c| (c.workspace.id, c.at.0, c.at.1));

    let mut windows = Vec::new();
    let mut browser = false;
    for c in sorted {
        if SKIP.contains(&c.class.as_str()) {
            continue;
        }
        let ws = c.workspace.id;
        if let Some(tok) = term::token_of(&c.initial_title) {
            let Some(k) = card(tok) else { continue };
            let float = c.class == "clios.float.term";
            windows.push(Win { workspace: ws, what: What::Term { cwd: k.cwd, run: safe_run(&k.cmd, &bins), float } });
            continue;
        }
        let id = c.class.strip_prefix("clios.tui.").or_else(|| c.class.strip_prefix("clios.float."));
        if let Some(id) = id {
            if catalog.tui.iter().any(|t| t.id == id) {
                windows.push(Win { workspace: ws, what: What::App { id: id.to_string() } });
            }
            continue;
        }
        if c.class == "firefox" && !browser {
            browser = true;
            windows.push(Win { workspace: ws, what: What::Gui { cmd: "firefox".into() } });
        }
    }
    Session { saved: now, windows }
}

/// A linha de shell que reabre uma janela. Usa o `foot` sem servidor: cada terminal vira um processo, e é pelo
/// processo que o Hyprland sabe em que workspace pôr a janela.
pub fn reopen_line(w: &What, catalog: &Catalog, theme: &clios_core::Theme, fish: bool, token: &str) -> Option<String> {
    match w {
        What::Term { cwd, run, float } => {
            let cwd = Path::new(cwd).is_dir().then(|| cwd.clone());
            let l = Launch {
                token: token.to_string(),
                cwd,
                run: run.clone(),
                class: float.then(|| "clios.float.term".to_string()),
                size: float.then(|| "100x30".to_string()),
                server: false,
                quiet: true,
            };
            Some(l.shell_line(fish))
        }
        What::App { id } => {
            let t = catalog.tui.iter().find(|t| t.id == *id && t.installed())?;
            let action = Action::Tui { id: t.id.clone(), argv: t.launch_argv(theme), float: t.float, hold: t.hold };
            let env = exec::Env { exe: "clios".into(), foot_server: false };
            let argv = exec::plan(&action, &env)?;
            // o plano começa com `setsid -f`, que mudaria o processo; aqui o Hyprland já desprende
            Some(argv[2..].iter().map(|a| crate::sys::sh_quote(a)).collect::<Vec<_>>().join(" "))
        }
        What::Gui { cmd } => is_installed(cmd).then(|| cmd.clone()),
    }
}

/// A expressão Lua que o `hyprctl dispatch` roda: o comando, na workspace, sem roubar o foco.
pub fn dispatch_expr(line: &str, workspace: i64) -> Option<String> {
    (!line.contains("]==]"))
        .then(|| format!("hl.dsp.exec_cmd([==[{line}]==], {{ workspace = \"{workspace} silent\" }})"))
}

fn file(ctx: &Ctx) -> PathBuf {
    ctx.paths.state.join("session.json")
}

pub fn load(ctx: &Ctx) -> Option<Session> {
    std::fs::read_to_string(file(ctx)).ok().and_then(|t| serde_json::from_str(&t).ok())
}

fn now() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs() as i64)
}

fn clients() -> Result<Vec<Client>> {
    let out = Command::new("hyprctl").args(["clients", "-j"]).output().context("hyprctl clients")?;
    anyhow::ensure!(out.status.success(), "o hyprctl não respondeu (o Hyprland está rodando?)");
    serde_json::from_slice(&out.stdout).context("lendo a resposta do hyprctl")
}

pub fn save(ctx: &Ctx, quiet: bool) -> Result<()> {
    let (catalog, _) = crate::catalog::load(&ctx.paths);
    let dir = term::dir();
    let card = |tok: &str| dir.as_deref().and_then(|d| term::read_card(d, tok));
    let s = capture(&clients()?, &card, &catalog, now());
    clios_core::fsutil::write_atomic(&file(ctx), serde_json::to_string_pretty(&s)?.as_bytes())?;
    if !quiet {
        println!("{} {}", ui::bold("sessão salva:"), count_words(s.windows.len()));
    }
    Ok(())
}

fn count_words(n: usize) -> String {
    match n {
        0 => "nenhuma janela".into(),
        1 => "uma janela".into(),
        n => format!("{n} janelas"),
    }
}

/// Descreve uma janela em uma linha, para o `show`.
pub fn describe(w: &What, home: &str) -> String {
    let tilde = |p: &str| match p.strip_prefix(home) {
        Some(rest) if !home.is_empty() => format!("~{rest}"),
        _ => p.to_string(),
    };
    match w {
        What::Term { cwd, run: Some(r), .. } => format!("{r}  {}", ui::dim(&format!("em {}", tilde(cwd)))),
        What::Term { cwd, run: None, .. } => format!("terminal  {}", ui::dim(&format!("em {}", tilde(cwd)))),
        What::App { id } => format!("{id}  {}", ui::dim("(catálogo)")),
        What::Gui { cmd } => cmd.clone(),
    }
}

pub fn show(ctx: &Ctx) -> Result<()> {
    let Some(s) = load(ctx) else {
        println!("nenhuma sessão salva ainda. Ela é salva sozinha a cada minuto, ou com `clios session save`.");
        return Ok(());
    };
    let home = ctx.paths.home.display().to_string();
    let ago = (now() - s.saved).max(0) as u64;
    println!("{} {}", ui::bold(&count_words(s.windows.len())), ui::dim(&format!("salva há {}", term::human_secs(ago))));
    let mut last = 0;
    for w in &s.windows {
        if w.workspace != last {
            println!("\n  {}", ui::dim(&format!("workspace {}", w.workspace)));
            last = w.workspace;
        }
        println!("    {}", describe(&w.what, &home));
    }
    if !ctx.state.session {
        println!("\n{}", ui::dim("reabrir no login está desligado: `clios session on` liga"));
    }
    Ok(())
}

/// Reabre. Com `login`, obedece à escolha do usuário e avisa por notificação em vez de imprimir.
pub fn restore(ctx: &Ctx, login: bool) -> Result<()> {
    if login && !ctx.state.session {
        return Ok(());
    }
    let Some(s) = load(ctx).filter(|s| !s.windows.is_empty()) else {
        if !login {
            println!("nada para reabrir");
        }
        return Ok(());
    };
    let (catalog, _) = crate::catalog::load(&ctx.paths);
    let theme = ctx.theme()?;
    let fish = is_installed("fish");
    let mut opened = 0;
    for w in &s.windows {
        let token = term::new_token();
        let Some(line) = reopen_line(&w.what, &catalog, &theme, fish, &token) else { continue };
        let Some(expr) = dispatch_expr(&line, w.workspace) else { continue };
        if Command::new("hyprctl").args(["dispatch", &expr]).output().is_ok_and(|o| o.status.success()) {
            opened += 1;
        }
        // um respiro entre janelas: o layout fica na ordem em que foram salvas
        std::thread::sleep(std::time::Duration::from_millis(60));
    }
    let msg = format!("{} de volta", count_words(opened));
    if login {
        let _ = Command::new("notify-send")
            .args(["-a", "clios", "-u", "low", "sessão reaberta", &format!("{msg}. `clios session off` desliga")])
            .status();
    } else {
        println!("{msg}");
    }
    Ok(())
}

pub fn set(ctx: &mut Ctx, on: bool) -> Result<()> {
    ctx.state.session = on;
    ctx.save_state()?;
    println!("{}", if on { "as janelas da última sessão voltam no login" } else { "o login começa com a mesa limpa" });
    Ok(())
}

pub fn forget(ctx: &Ctx) -> Result<()> {
    match std::fs::remove_file(file(ctx)) {
        Ok(()) => println!("sessão esquecida"),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => println!("não havia sessão salva"),
        Err(e) => return Err(e).context("apagando a sessão"),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog;

    fn c(class: &str, title: &str, ws: i64, x: i64) -> Client {
        Client {
            class: class.into(),
            initial_title: title.into(),
            workspace: ClientWs { id: ws },
            at: (x, 0),
            mapped: true,
        }
    }

    fn cards(tok: &str) -> Option<Card> {
        match tok {
            "a1" => Some(Card { cwd: "/home/k/site".into(), cmd: "hx index.html".into() }),
            "b2" => Some(Card { cwd: "/home/k".into(), cmd: "cargo build --release".into() }),
            "c3" => Some(Card { cwd: "/tmp".into(), cmd: String::new() }),
            _ => None,
        }
    }

    #[test]
    fn interactive_programs_come_back_and_everything_else_stays_a_shell() {
        let bins = ["yazi", "lazygit"];
        assert_eq!(safe_run("hx src/main.rs", &bins).as_deref(), Some("hx src/main.rs"));
        assert_eq!(safe_run("yazi", &bins).as_deref(), Some("yazi"));
        assert_eq!(safe_run("rm -rf build", &bins), None);
        assert_eq!(safe_run("cargo build", &bins), None);
        assert_eq!(safe_run("hx $(fd x)", &bins), None);
        assert_eq!(safe_run("less log | head", &bins), None);
        assert_eq!(safe_run("hx a; rm b", &bins), None);
        assert_eq!(safe_run("   ", &bins), None);
    }

    #[test]
    fn capture_keeps_order_skips_the_shell_parts_and_the_browser_comes_once() {
        let cat = catalog::builtin();
        let clients = vec![
            c("foot", "clios-term:b2", 2, 0),
            c("firefox", "Mozilla Firefox", 3, 0),
            c("firefox", "Mozilla Firefox", 3, 900),
            c("foot", "clios-term:a1", 1, 960),
            c("clios.tui.git", "lazygit", 1, 0),
            c("clios.hub", "clios-term:zz", 1, 0),
            c("clios.scratch", "", 1, 0),
            c("foot", "clios-term:c3", -98, 0),
            c("foot", "sem ficha", 4, 0),
            c("clios.tui.naoexiste", "", 4, 0),
        ];
        let s = capture(&clients, &cards, &cat, 7);
        assert_eq!(s.saved, 7);
        assert_eq!(
            s.windows,
            vec![
                Win { workspace: 1, what: What::App { id: "git".into() } },
                Win {
                    workspace: 1,
                    what: What::Term { cwd: "/home/k/site".into(), run: Some("hx index.html".into()), float: false }
                },
                Win { workspace: 2, what: What::Term { cwd: "/home/k".into(), run: None, float: false } },
                Win { workspace: 3, what: What::Gui { cmd: "firefox".into() } },
            ]
        );
    }

    #[test]
    fn the_session_file_is_stable_json() {
        let s = Session {
            saved: 1,
            windows: vec![
                Win { workspace: 2, what: What::Term { cwd: "/p".into(), run: None, float: true } },
                Win { workspace: 1, what: What::App { id: "files".into() } },
            ],
        };
        let j = serde_json::to_string(&s).unwrap();
        assert!(j.contains(r#"{"workspace":2,"kind":"term","cwd":"/p","float":true}"#), "{j}");
        assert_eq!(serde_json::from_str::<Session>(&j).unwrap(), s);
    }

    #[test]
    fn a_terminal_comes_back_in_its_folder_with_its_own_process_and_without_a_greeting() {
        let cat = catalog::builtin();
        let theme = crate::hub::app::tests::theme(clios_core::MotionLevel::Full);
        let w = What::Term { cwd: "/".into(), run: Some("hx x".into()), float: false };
        let line = reopen_line(&w, &cat, &theme, true, "t1").unwrap();
        assert_eq!(
            line,
            "env CLIOS_TERM=t1 CLIOS_GREETED=1 foot --title=clios-term:t1 --working-directory=/ fish -C 'hx x'"
        );
        // pasta que sumiu: abre na pasta pessoal
        let gone = What::Term { cwd: "/nao/existe/mais".into(), run: None, float: false };
        assert!(!reopen_line(&gone, &cat, &theme, true, "t2").unwrap().contains("working-directory"));
    }

    #[test]
    fn the_dispatch_puts_the_window_in_its_workspace_silently() {
        assert_eq!(
            dispatch_expr("foot -D '/a'", 3).unwrap(),
            "hl.dsp.exec_cmd([==[foot -D '/a']==], { workspace = \"3 silent\" })"
        );
        assert_eq!(dispatch_expr("x ]==] y", 1), None);
    }

    #[test]
    fn describe_uses_tilde() {
        let w = What::Term { cwd: "/home/k/site".into(), run: None, float: false };
        assert!(describe(&w, "/home/k").contains("~/site"));
        assert!(describe(&What::App { id: "git".into() }, "/home/k").starts_with("git"));
    }
}
