//! `clios term`: abre um terminal que o CLIOS reconhece depois.
//!
//! Cada terminal nasce com uma ficha (`CLIOS_TERM`) e com ela no título inicial (`clios-term:<ficha>`). O título muda
//! logo depois, mas o Hyprland guarda o inicial (`initialTitle`), e é assim que uma janela leva à sua ficha. O fish
//! escreve na ficha, sem abrir processo nenhum, a pasta e o comando que está rodando (`$XDG_RUNTIME_DIR/clios/term`).
//! Com isso dá para abrir um terminal na mesma pasta do que está em foco, avisar quando um comando longo termina num
//! terminal que você não está olhando e, no `clios session`, reabrir cada terminal onde ele estava.

use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};

use crate::sys::{is_installed, sh_quote};

/// O prefixo do título inicial de um terminal do CLIOS.
pub const TITLE_PREFIX: &str = "clios-term:";

/// Uma ficha nova: 10 dígitos hex, do relógio e do pid (não precisa ser segredo, só não repetir).
pub fn new_token() -> String {
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_nanos());
    let mixed = (nanos as u64) ^ (u64::from(std::process::id()) << 40) ^ (nanos >> 64) as u64;
    format!("{:010x}", mixed.wrapping_mul(0x9E37_79B9_7F4A_7C15) >> 24)
}

/// A ficha de uma janela, pelo título inicial.
pub fn token_of(initial_title: &str) -> Option<&str> {
    initial_title.strip_prefix(TITLE_PREFIX).filter(|t| !t.is_empty() && t.chars().all(|c| c.is_ascii_alphanumeric()))
}

/// Onde o fish escreve as fichas.
pub fn dir() -> Option<PathBuf> {
    let rt = std::env::var_os("XDG_RUNTIME_DIR").map(PathBuf::from).filter(|p| p.is_absolute())?;
    Some(rt.join("clios/term"))
}

/// O que o fish contou sobre um terminal.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Card {
    pub cwd: String,
    /// A linha de comando em execução; vazia quando o shell está esperando no prompt.
    pub cmd: String,
}

impl Card {
    /// Linhas `chave<TAB>valor`; chaves desconhecidas são ignoradas (o formato pode crescer).
    pub fn parse(text: &str) -> Self {
        let mut c = Card::default();
        for line in text.lines() {
            let Some((k, v)) = line.split_once('\t') else { continue };
            match k {
                "cwd" => c.cwd = v.to_string(),
                "cmd" => c.cmd = v.trim().to_string(),
                _ => {}
            }
        }
        c
    }
}

pub fn read_card(dir: &Path, token: &str) -> Option<Card> {
    std::fs::read_to_string(dir.join(token)).ok().map(|t| Card::parse(&t)).filter(|c| !c.cwd.is_empty())
}

/// Como abrir um terminal.
#[derive(Debug, Clone, Default)]
pub struct Launch {
    pub token: String,
    pub cwd: Option<String>,
    /// Um comando para rodar dentro do fish (que continua aberto quando ele termina).
    pub run: Option<String>,
    /// Classe da janela (app-id do foot): `clios.float.term` para o flutuante.
    pub class: Option<String>,
    pub size: Option<String>,
    /// `footclient` (instantâneo, mas todas as janelas são do mesmo processo) ou `foot` (um processo próprio, que o
    /// Hyprland consegue mandar para uma workspace pela regra do `exec`).
    pub server: bool,
    /// Não repete a apresentação (`clios greet`): é um terminal reaberto, não um novo.
    pub quiet: bool,
}

impl Launch {
    /// Os argumentos, prontos para executar. As variáveis vão por `env` porque o `footclient` só repassa o ambiente
    /// com `-E`, e o `foot` herda o do processo.
    pub fn argv(&self, fish: bool) -> Vec<String> {
        let mut v: Vec<String> = vec!["env".into(), format!("CLIOS_TERM={}", self.token)];
        if self.quiet {
            v.push("CLIOS_GREETED=1".into());
        }
        if self.server {
            v.extend(["footclient".into(), "-E".into()]);
        } else {
            v.push("foot".into());
        }
        v.push(format!("--title={TITLE_PREFIX}{}", self.token));
        if let Some(class) = &self.class {
            v.push(format!("--app-id={class}"));
        }
        if let Some(size) = &self.size {
            v.push(format!("--window-size-chars={size}"));
        }
        if let Some(cwd) = &self.cwd {
            v.push(format!("--working-directory={cwd}"));
        }
        if let Some(run) = &self.run {
            if fish {
                v.extend(["fish".into(), "-C".into(), run.clone()]);
            } else {
                v.extend(["sh".into(), "-c".into(), run.clone()]);
            }
        }
        v
    }

    pub fn shell_line(&self, fish: bool) -> String {
        self.argv(fish).iter().map(|a| sh_quote(a)).collect::<Vec<_>>().join(" ")
    }
}

/// A janela em foco, do `hyprctl activewindow -j`: (título inicial, classe).
pub fn parse_active(json: &str) -> Option<(String, String)> {
    let v: serde_json::Value = serde_json::from_str(json).ok()?;
    Some((v["initialTitle"].as_str().unwrap_or_default().to_string(), v["class"].as_str().unwrap_or_default().into()))
}

fn active() -> Option<(String, String)> {
    let out = Command::new("hyprctl").args(["activewindow", "-j"]).output().ok()?;
    parse_active(&String::from_utf8_lossy(&out.stdout))
}

/// A pasta do terminal em foco, se ele for um terminal do CLIOS e a pasta ainda existir.
pub fn focused_cwd() -> Option<String> {
    let (title, _) = active()?;
    let card = read_card(&dir()?, token_of(&title)?)?;
    Path::new(&card.cwd).is_dir().then_some(card.cwd)
}

fn foot_server_running() -> bool {
    let Some(rt) = std::env::var_os("XDG_RUNTIME_DIR") else { return false };
    let display = std::env::var("WAYLAND_DISPLAY").unwrap_or_default();
    let name = if display.is_empty() { "foot.sock".to_string() } else { format!("foot-{display}.sock") };
    Path::new(&rt).join(name).exists()
}

pub struct Options {
    pub here: bool,
    pub float: bool,
    pub cwd: Option<PathBuf>,
    pub run: Vec<String>,
}

/// Abre o terminal. Substitui este processo pelo do terminal (o atalho não fica com um `clios` pendurado).
pub fn open(opts: Options) -> Result<()> {
    let cwd = match (opts.cwd, opts.here) {
        (Some(p), _) => Some(p.display().to_string()),
        (None, true) => focused_cwd(),
        (None, false) => None,
    };
    let launch = Launch {
        token: new_token(),
        cwd,
        run: (!opts.run.is_empty()).then(|| opts.run.join(" ")),
        class: opts.float.then(|| "clios.float.term".to_string()),
        size: opts.float.then(|| "100x30".to_string()),
        server: foot_server_running(),
        quiet: false,
    };
    let argv = launch.argv(is_installed("fish"));
    let err = Command::new(&argv[0]).args(&argv[1..]).exec();
    Err(err).with_context(|| format!("não consegui abrir o terminal ({})", argv[2..].join(" ")))
}

// ── comando longo ─────────────────────────────────────────────────────────

/// `1m 12s`, `45s`, `2h 03m`.
pub fn human_secs(s: u64) -> String {
    match s {
        0..=59 => format!("{s}s"),
        60..=3599 => format!("{}m {:02}s", s / 60, s % 60),
        _ => format!("{}h {:02}m", s / 3600, (s % 3600) / 60),
    }
}

/// Avisa se valer a pena: só quando o terminal do comando não está em foco (quem está olhando já viu).
pub fn done(secs: u64, status: i32, cmd: &str) -> Result<()> {
    let me = std::env::var("CLIOS_TERM").unwrap_or_default();
    if let Some((title, _)) = active() {
        if !me.is_empty() && token_of(&title) == Some(me.as_str()) {
            return Ok(());
        }
    }
    let short: String = cmd.chars().take(60).collect();
    let (title, urgency) = if status == 0 {
        (format!("pronto em {}", human_secs(secs)), "normal")
    } else {
        (format!("falhou ({status}) depois de {}", human_secs(secs)), "critical")
    };
    Command::new("notify-send")
        .args(["-a", "terminal", "-u", urgency, &title, &short])
        .status()
        .context("notify-send")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokens_are_short_hex_and_do_not_repeat() {
        let a = new_token();
        std::thread::sleep(std::time::Duration::from_millis(1));
        let b = new_token();
        assert_eq!(a.len(), 10);
        assert!(a.chars().all(|c| c.is_ascii_hexdigit()));
        assert_ne!(a, b);
    }

    #[test]
    fn the_token_comes_from_the_initial_title_only() {
        assert_eq!(token_of("clios-term:ab12cd"), Some("ab12cd"));
        assert_eq!(token_of("clios-term:"), None);
        assert_eq!(token_of("clios-term:../x"), None);
        assert_eq!(token_of("~/projeto — hx"), None);
    }

    #[test]
    fn cards_parse_and_ignore_unknown_keys() {
        let c = Card::parse("cwd\t/home/k/código\ncmd\thx src/main.rs \nnovo\tx\n");
        assert_eq!(c, Card { cwd: "/home/k/código".into(), cmd: "hx src/main.rs".into() });
        assert_eq!(Card::parse("lixo"), Card::default());
    }

    #[test]
    fn launch_with_the_server_passes_the_environment_and_the_title() {
        let l = Launch { token: "abc".into(), cwd: Some("/tmp/a b".into()), server: true, ..Default::default() };
        assert_eq!(
            l.argv(true),
            ["env", "CLIOS_TERM=abc", "footclient", "-E", "--title=clios-term:abc", "--working-directory=/tmp/a b"]
        );
    }

    #[test]
    fn a_reopened_terminal_is_quiet_runs_its_command_in_fish_and_has_its_own_process() {
        let l = Launch {
            token: "f00".into(),
            cwd: Some("/p".into()),
            run: Some("hx 'a b.rs'".into()),
            quiet: true,
            ..Default::default()
        };
        assert_eq!(
            l.argv(true),
            [
                "env",
                "CLIOS_TERM=f00",
                "CLIOS_GREETED=1",
                "foot",
                "--title=clios-term:f00",
                "--working-directory=/p",
                "fish",
                "-C",
                "hx 'a b.rs'"
            ]
        );
        assert!(l.shell_line(false).ends_with(r"sh -c 'hx '\''a b.rs'\'''"), "{}", l.shell_line(false));
    }

    #[test]
    fn floating_terminal_keeps_its_class_and_size() {
        let l = Launch {
            token: "1".into(),
            class: Some("clios.float.term".into()),
            size: Some("100x30".into()),
            server: true,
            ..Default::default()
        };
        let v = l.argv(true);
        assert!(v.contains(&"--app-id=clios.float.term".to_string()));
        assert!(v.contains(&"--window-size-chars=100x30".to_string()));
    }

    #[test]
    fn active_window_json() {
        let j = r#"{"class":"foot","initialTitle":"clios-term:9a","title":"~"}"#;
        assert_eq!(parse_active(j), Some(("clios-term:9a".into(), "foot".into())));
        assert_eq!(parse_active("{}"), Some((String::new(), String::new())));
        assert_eq!(parse_active("não é json"), None);
    }

    #[test]
    fn durations_read_like_a_person_would_say_them() {
        assert_eq!(human_secs(9), "9s");
        assert_eq!(human_secs(72), "1m 12s");
        assert_eq!(human_secs(7380), "2h 03m");
    }
}
