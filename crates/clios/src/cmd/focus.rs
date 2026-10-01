//! `clios focus`: um bloco de foco com hora para acabar. Liga o "não perturbe", a barra mostra quanto falta, e no
//! fim o CLIOS avisa (uma notificação normal, que passa mesmo com o silêncio ainda ligado) e devolve o silêncio
//! ao que era antes. É o pomodoro, sem app: um processo discreto espera o tempo passar.

use std::fs;
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::{Result, bail};

use super::bg::Bg;
use super::toggles::{self, Switch};
use crate::ctx::Ctx;
use crate::ui;

pub const FOCUS: Bg = Bg { name: "focus", needle: "focus --wait" };
pub const DEFAULT_MINUTES: u64 = 25;
pub const MAX_MINUTES: u64 = 240;

/// O bloco em andamento: quando acaba (segundos desde 1970) e se o silêncio já estava ligado antes dele.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Session {
    pub end: u64,
    pub dnd_before: bool,
}

impl Session {
    pub fn encode(self) -> String {
        format!("{}\n{}\n", self.end, u8::from(self.dnd_before))
    }

    pub fn decode(text: &str) -> Option<Session> {
        let mut l = text.lines();
        let end = l.next()?.trim().parse().ok()?;
        let dnd_before = l.next().map(str::trim) == Some("1");
        Some(Session { end, dnd_before })
    }

    /// Segundos que faltam em `now`.
    pub fn left(self, now: u64) -> u64 {
        self.end.saturating_sub(now)
    }
}

/// `25`, `50` ou `1h`... só minutos inteiros, de 1 a 240.
pub fn parse_minutes(s: &str) -> Result<u64> {
    let n: u64 =
        s.trim().trim_end_matches('m').parse().map_err(|_| anyhow::anyhow!("{s:?} não é um número de minutos"))?;
    if !(1..=MAX_MINUTES).contains(&n) {
        bail!("{n} minutos fora do intervalo (1 a {MAX_MINUTES})");
    }
    Ok(n)
}

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

fn file(ctx: &Ctx) -> PathBuf {
    ctx.paths.state.join("focus")
}

fn read(ctx: &Ctx) -> Option<Session> {
    Session::decode(&fs::read_to_string(file(ctx)).ok()?)
}

/// Segundos que faltam, ou `None` se não há bloco (ou o processo que o controla morreu).
pub fn remaining(ctx: &Ctx) -> Option<u64> {
    let s = read(ctx)?;
    if !ctx.sandboxed && !FOCUS.running(ctx) {
        return None;
    }
    Some(s.left(now())).filter(|l| *l > 0)
}

fn notify(title: &str, body: &str, urgency: &str) {
    let _ = std::process::Command::new("notify-send")
        .args(["-a", "clios", "-u", urgency, title, body])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();
}

pub fn start(ctx: &Ctx, minutes: u64) -> Result<()> {
    // Recomeçar no meio de um bloco não pode esquecer o silêncio de antes dele.
    let dnd_before = read(ctx).map_or_else(|| toggles::dnd_on(ctx), |s| s.dnd_before);
    if FOCUS.running(ctx) {
        FOCUS.stop(ctx, "TERM")?;
    }
    let session = Session { end: now() + minutes * 60, dnd_before };
    clios_core::fsutil::write_atomic(&file(ctx), session.encode().as_bytes())?;
    toggles::dnd(ctx, Switch::On)?;
    if !ctx.sandboxed {
        let exe = std::env::current_exe()?.display().to_string();
        FOCUS.start(ctx, &exe, &["focus".into(), "--wait".into()])?;
    }
    println!("foco por {minutes} min {}", ui::dim("(clios focus para parar)"));
    Ok(())
}

/// Encerra o bloco: devolve o silêncio ao que era e, se o tempo acabou, avisa.
pub fn finish(ctx: &Ctx, minutes_done: Option<u64>) -> Result<()> {
    if let Some(s) = read(ctx) {
        if !s.dnd_before {
            toggles::dnd(ctx, Switch::Off)?;
        }
    }
    let _ = fs::remove_file(file(ctx));
    if let Some(m) = minutes_done {
        notify("o foco terminou", &format!("{m} minutos. Hora de uma pausa."), "normal");
    }
    Ok(())
}

pub fn stop(ctx: &Ctx) -> Result<()> {
    let running = FOCUS.running(ctx);
    if running {
        FOCUS.stop(ctx, "TERM")?;
    }
    if read(ctx).is_none() && !running {
        println!("não há um bloco de foco em andamento");
        return Ok(());
    }
    finish(ctx, None)?;
    println!("foco encerrado");
    Ok(())
}

/// O processo de segundo plano: espera o fim e avisa. Roda como `clios focus --wait`.
pub fn wait(ctx: &Ctx) -> Result<()> {
    let Some(s) = read(ctx) else { return Ok(()) };
    let total = s.left(now());
    std::thread::sleep(Duration::from_secs(total));
    // O arquivo pode ter mudado (alguém recomeçou o bloco): só encerra se ainda é este.
    if read(ctx) == Some(s) {
        let minutes = (total + 30) / 60;
        finish(ctx, Some(minutes.max(1)))?;
    }
    Ok(())
}

/// `clios focus` sem argumento: se há bloco, para; senão começa um de 25 minutos.
pub fn run(ctx: &Ctx, arg: Option<&str>) -> Result<()> {
    match arg {
        Some("stop" | "parar" | "off") => stop(ctx),
        Some(m) => start(ctx, parse_minutes(m)?),
        None if remaining(ctx).is_some() => stop(ctx),
        None => start(ctx, DEFAULT_MINUTES),
    }
}

/// `clios focus status`: o tempo que falta, em texto (para scripts).
pub fn status_text(ctx: &Ctx) -> String {
    match remaining(ctx) {
        Some(s) => format!("{}m", s.div_ceil(60)),
        None => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn ctx(name: &str) -> Ctx {
        let home = std::env::temp_dir().join(format!("clios-focus-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&home);
        fs::create_dir_all(&home).unwrap();
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        Ctx::load(Some(&root), Some(&home)).unwrap()
    }

    #[test]
    fn minutes_are_parsed_strictly() {
        assert_eq!(parse_minutes("25").unwrap(), 25);
        assert_eq!(parse_minutes(" 50m ").unwrap(), 50);
        assert!(parse_minutes("0").is_err());
        assert!(parse_minutes("241").is_err());
        assert!(parse_minutes("meia hora").unwrap_err().to_string().contains("não é um número"));
    }

    #[test]
    fn session_roundtrips_and_counts_down() {
        let s = Session { end: 1_000, dnd_before: true };
        assert_eq!(Session::decode(&s.encode()), Some(s));
        assert_eq!(s.left(400), 600);
        assert_eq!(s.left(2_000), 0, "passou do fim: zero, sem estourar");
        assert_eq!(Session::decode("lixo"), None);
        assert_eq!(Session::decode("77\n"), Some(Session { end: 77, dnd_before: false }));
    }

    #[test]
    fn a_block_turns_the_silence_on_and_ends_by_putting_it_back() {
        let c = ctx("block");
        assert!(!toggles::dnd_on(&c));
        start(&c, 25).unwrap();
        assert!(toggles::dnd_on(&c), "o foco liga o não perturbe");
        let left = remaining(&c).unwrap();
        assert!((24 * 60..=25 * 60).contains(&left), "{left}");
        assert_eq!(status_text(&c), "25m");
        stop(&c).unwrap();
        assert!(!toggles::dnd_on(&c), "estava desligado antes: volta desligado");
        assert_eq!(remaining(&c), None);
    }

    #[test]
    fn a_silence_that_was_already_on_stays_on_after_the_block() {
        let c = ctx("kept");
        toggles::dnd(&c, Switch::On).unwrap();
        start(&c, 10).unwrap();
        start(&c, 15).unwrap(); // recomeçar não pode esquecer o "antes"
        stop(&c).unwrap();
        assert!(toggles::dnd_on(&c), "quem já estava em silêncio continua em silêncio");
    }

    #[test]
    fn no_argument_toggles() {
        let c = ctx("toggle");
        run(&c, None).unwrap();
        assert!(remaining(&c).is_some());
        run(&c, None).unwrap();
        assert!(remaining(&c).is_none());
        run(&c, Some("5")).unwrap();
        assert!(status_text(&c).ends_with('m'));
        run(&c, Some("stop")).unwrap();
        assert_eq!(status_text(&c), "");
    }
}
