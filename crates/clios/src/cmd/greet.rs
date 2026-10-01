//! `clios greet`: o terminal se apresenta, mas só quando faz sentido.
//!
//! O fish chama isto ao abrir. Quem decide é uma função pura (`decide`):
//!   - o primeiro terminal depois de ligar mostra o resumo do sistema (`clios fetch`);
//!   - o primeiro terminal de cada workspace vazia mostra duas linhas discretas: a saudação e uma dica;
//!   - o resto do tempo, silêncio. O minimalismo é o padrão.
//!
//! O que já foi mostrado fica em `$XDG_RUNTIME_DIR/clios/greeted`, que some a cada boot.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use anyhow::Result;
use clios_core::GreetMode;
use clios_core::fsutil::write_atomic;

use crate::ctx::Ctx;
use crate::ui;
use crate::welcome::data;
use crate::{cmd::fetch, sysinfo};

/// O que mostrar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum Greeting {
    /// Nada.
    #[value(skip)]
    Nothing,
    /// O resumo do sistema com a marca.
    Fetch,
    /// A saudação e uma dica, em duas linhas.
    Line,
}

/// O que já foi mostrado nesta sessão.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Seen {
    pub boot: bool,
    pub workspaces: BTreeSet<i64>,
}

impl Seen {
    pub fn parse(text: &str) -> Seen {
        let mut s = Seen::default();
        for line in text.lines().map(str::trim) {
            if line == "boot" {
                s.boot = true;
            } else if let Some(id) = line.strip_prefix("ws:").and_then(|n| n.parse().ok()) {
                s.workspaces.insert(id);
            }
        }
        s
    }

    pub fn serialize(&self) -> String {
        let mut out = String::new();
        if self.boot {
            out.push_str("boot\n");
        }
        for w in &self.workspaces {
            out.push_str(&format!("ws:{w}\n"));
        }
        out
    }
}

/// O que dá para saber da janela deste terminal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Facts {
    /// A classe (app-id) da janela ativa. Só o terminal comum, `foot`, é apresentado: o do scratchpad,
    /// as janelas flutuantes e os apps de terminal (`clios.*`) nunca.
    pub class: String,
    pub workspace: Option<i64>,
    /// Quantas janelas a workspace tem, contando este terminal.
    pub windows_here: usize,
}

pub fn decide(mode: GreetMode, seen: &Seen, f: &Facts) -> Greeting {
    if mode == GreetMode::Off || f.class != "foot" {
        return Greeting::Nothing;
    }
    if !seen.boot {
        return Greeting::Fetch;
    }
    match f.workspace {
        Some(ws) if mode == GreetMode::All && !seen.workspaces.contains(&ws) && f.windows_here <= 1 => Greeting::Line,
        _ => Greeting::Nothing,
    }
}

/// Registra o que foi mostrado.
pub fn mark(seen: &mut Seen, shown: Greeting, f: &Facts) {
    if shown == Greeting::Fetch {
        seen.boot = true;
    }
    if shown != Greeting::Nothing {
        seen.workspaces.extend(f.workspace);
    }
}

/// `{"class":"foot","workspace":{"id":3}}` do `hyprctl activewindow -j`.
pub fn parse_active(json: &str) -> Option<(String, i64)> {
    let v: serde_json::Value = serde_json::from_str(json).ok()?;
    Some((v["class"].as_str()?.to_string(), v["workspace"]["id"].as_i64()?))
}

/// Quantas janelas (mapeadas) há na workspace, do `hyprctl clients -j`.
pub fn count_on_workspace(json: &str, ws: i64) -> usize {
    let Ok(serde_json::Value::Array(all)) = serde_json::from_str::<serde_json::Value>(json) else { return 0 };
    all.iter().filter(|c| c["mapped"].as_bool().unwrap_or(true) && c["workspace"]["id"].as_i64() == Some(ws)).count()
}

fn hyprctl(args: &[&str]) -> Option<String> {
    let out = Command::new("hyprctl").args(args).output().ok()?;
    out.status.success().then(|| String::from_utf8_lossy(&out.stdout).into_owned())
}

/// Lê a janela ativa. O shell abre alguns milissegundos antes da janela estar pronta, então tenta de novo.
fn facts() -> Option<Facts> {
    for attempt in 0..4 {
        if attempt > 0 {
            std::thread::sleep(Duration::from_millis(80));
        }
        let Some((class, ws)) = hyprctl(&["activewindow", "-j"]).and_then(|j| parse_active(&j)) else { continue };
        let here = hyprctl(&["clients", "-j"]).map_or(0, |j| count_on_workspace(&j, ws));
        return Some(Facts { class, workspace: Some(ws), windows_here: here });
    }
    None
}

fn seen_path() -> Option<PathBuf> {
    let rt = std::env::var_os("XDG_RUNTIME_DIR").map(PathBuf::from).filter(|p| p.is_absolute())?;
    Some(rt.join("clios/greeted"))
}

fn load_seen(path: &Path) -> Seen {
    std::fs::read_to_string(path).map(|t| Seen::parse(&t)).unwrap_or_default()
}

/// Duas linhas: a saudação e uma dica do baralho do guia, no formato `dica  <ideia>  <atalho>`.
pub fn line(ctx: &Ctx, hour: u32, user: &str, tip_seed: usize) -> Result<String> {
    let theme = ctx.theme()?;
    let tips = data::tips();
    let tip = &tips[tip_seed % tips.len()];
    let paint = |s: &str, c: clios_core::Rgb| format!("\x1b[38;2;{};{};{}m{s}\x1b[0m", c.r, c.g, c.b);
    let hello = if user.is_empty() {
        sysinfo::greeting(hour).to_string()
    } else {
        format!("{}, {user}", sysinfo::greeting(hour))
    };
    let mut out = format!("  {}\n", paint(&hello, theme.c.dim));
    out.push_str(&format!("  {}  {}", paint("dica", theme.c.mute), paint(&tip.title, theme.c.dim)));
    if let Some(keys) = &tip.keys {
        out.push_str(&format!("  {}", paint(keys, theme.c.accent)));
    }
    out.push('\n');
    Ok(out)
}

pub fn show(ctx: &Ctx, what: Greeting) -> Result<()> {
    match what {
        Greeting::Nothing => Ok(()),
        Greeting::Fetch => fetch::run(ctx),
        Greeting::Line => {
            let hour = sysinfo::local_hour().unwrap_or(12);
            let user = std::env::var("USER").unwrap_or_default();
            let seed = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.subsec_nanos() as usize);
            let text = line(ctx, hour, &user, seed)?;
            print!("{text}");
            Ok(())
        }
    }
}

pub fn describe(m: GreetMode) -> &'static str {
    match m {
        GreetMode::All => "ao ligar, e uma linha na primeira vez que uma workspace vazia recebe um terminal",
        GreetMode::Boot => "só no primeiro terminal depois de ligar",
        GreetMode::Off => "desligada",
    }
}

/// Sem `--show`, decide sozinho e grava o que mostrou. Com `--show`, mostra e pronto (para ver como fica).
pub fn run(ctx: &Ctx, force: Option<Greeting>, reset: bool) -> Result<()> {
    let path = seen_path();
    if reset {
        if let Some(p) = &path {
            let _ = std::fs::remove_file(p);
        }
        return Ok(());
    }
    if let Some(g) = force {
        return show(ctx, g);
    }
    // Só terminal de verdade, dentro do Hyprland, fora de ssh: é o que o fish já garante, e aqui de novo.
    if !ui::color_enabled() || std::env::var_os("HYPRLAND_INSTANCE_SIGNATURE").is_none() || ctx.sandboxed {
        return Ok(());
    }
    let Some(path) = path else { return Ok(()) };
    let Some(f) = facts() else { return Ok(()) };
    let mut seen = load_seen(&path);
    let g = decide(ctx.state.greet, &seen, &f);
    if g == Greeting::Nothing {
        return Ok(());
    }
    mark(&mut seen, g, &f);
    let _ = write_atomic(&path, seen.serialize().as_bytes());
    show(ctx, g)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn foot(ws: i64, n: usize) -> Facts {
        Facts { class: "foot".into(), workspace: Some(ws), windows_here: n }
    }

    #[test]
    fn the_first_terminal_after_boot_gets_the_fetch() {
        assert_eq!(decide(GreetMode::All, &Seen::default(), &foot(1, 1)), Greeting::Fetch);
        assert_eq!(
            decide(GreetMode::Boot, &Seen::default(), &foot(1, 3)),
            Greeting::Fetch,
            "no boot, mesmo com a workspace cheia"
        );
    }

    #[test]
    fn after_that_only_an_empty_workspace_that_was_never_greeted_gets_the_line() {
        let mut seen = Seen { boot: true, ..Seen::default() };
        assert_eq!(decide(GreetMode::All, &seen, &foot(2, 1)), Greeting::Line);
        assert_eq!(
            decide(GreetMode::All, &seen, &foot(2, 2)),
            Greeting::Nothing,
            "já tem janela: não é workspace vazia"
        );
        seen.workspaces.insert(2);
        assert_eq!(decide(GreetMode::All, &seen, &foot(2, 1)), Greeting::Nothing, "uma vez por workspace");
        assert_eq!(decide(GreetMode::All, &seen, &foot(3, 1)), Greeting::Line);
    }

    #[test]
    fn boot_mode_never_shows_the_line_and_off_shows_nothing() {
        let seen = Seen { boot: true, ..Seen::default() };
        assert_eq!(decide(GreetMode::Boot, &seen, &foot(2, 1)), Greeting::Nothing);
        assert_eq!(decide(GreetMode::Off, &Seen::default(), &foot(1, 1)), Greeting::Nothing);
    }

    #[test]
    fn only_the_plain_terminal_is_greeted() {
        for class in ["clios.scratch", "clios.float.term", "clios.hub", "clios.tui.files", "firefox", ""] {
            let f = Facts { class: class.into(), workspace: Some(1), windows_here: 1 };
            assert_eq!(decide(GreetMode::All, &Seen::default(), &f), Greeting::Nothing, "{class:?}");
        }
    }

    #[test]
    fn marking_the_fetch_also_covers_the_workspace_so_no_line_follows_it() {
        let mut seen = Seen::default();
        let f = foot(4, 1);
        mark(&mut seen, Greeting::Fetch, &f);
        assert!(seen.boot && seen.workspaces.contains(&4));
        assert_eq!(decide(GreetMode::All, &seen, &foot(4, 1)), Greeting::Nothing);
        mark(&mut seen, Greeting::Nothing, &foot(9, 1));
        assert!(!seen.workspaces.contains(&9), "o que não foi mostrado não se marca");
    }

    #[test]
    fn seen_roundtrips_and_ignores_garbage() {
        let s = Seen { boot: true, workspaces: [3, 1].into() };
        assert_eq!(s.serialize(), "boot\nws:1\nws:3\n");
        assert_eq!(Seen::parse(&s.serialize()), s);
        assert_eq!(Seen::parse("lixo\nws:x\n\n"), Seen::default());
    }

    #[test]
    fn hyprctl_output_is_parsed() {
        let active = r#"{"address":"0x1","mapped":true,"class":"foot","title":"fish","workspace":{"id":3,"name":"3"}}"#;
        assert_eq!(parse_active(active), Some(("foot".into(), 3)));
        assert_eq!(parse_active("{}"), None, "sem janela ativa");
        assert_eq!(parse_active("Invalid"), None);
        let clients = r#"[{"mapped":true,"workspace":{"id":3}},{"mapped":true,"workspace":{"id":2}},{"mapped":false,"workspace":{"id":3}}]"#;
        assert_eq!(count_on_workspace(clients, 3), 1, "a janela que ainda não abriu não conta");
        assert_eq!(count_on_workspace(clients, 9), 0);
        assert_eq!(count_on_workspace("lixo", 3), 0);
    }
}
