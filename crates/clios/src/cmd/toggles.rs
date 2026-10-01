//! Os pequenos liga-desliga do dia a dia: café, modo noturno, não perturbe, gravação, conta-gotas e energia.
//! Todos avisam com uma notificação (a que a barra mostra), e `status` resume o que está ligado.

use std::fs;
use std::path::PathBuf;
use std::process::{Command, Stdio};

use anyhow::{Context, Result, bail};
use clap::ValueEnum;
use serde::Serialize;

use super::bg::Bg;
use super::status::{self, Net};
use crate::ctx::Ctx;

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum Switch {
    On,
    Off,
    Toggle,
}

impl Switch {
    /// O estado que sai de aplicar isto sobre o estado atual.
    pub fn resolve(self, current: bool) -> bool {
        match self {
            Switch::On => true,
            Switch::Off => false,
            Switch::Toggle => !current,
        }
    }
}

pub const CAFFEINE: Bg = Bg { name: "caffeine", needle: "systemd-inhibit" };
pub const NIGHT: Bg = Bg { name: "night", needle: "hyprsunset" };
pub const REC: Bg = Bg { name: "rec", needle: "wf-recorder" };

fn notify(title: &str, body: &str) {
    let _ = Command::new("notify-send")
        .args(["-a", "clios", "-u", "low", title, body])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
}

// ── café ──────────────────────────────────────────────────────────────────

/// Os argumentos do `systemd-inhibit` que seguram a tela acesa e o sistema acordado.
pub fn caffeine_args() -> Vec<String> {
    ["--what=idle:sleep", "--who=clios", "--why=modo café", "--mode=block", "sleep", "infinity"]
        .map(String::from)
        .to_vec()
}

/// Liga, desliga ou alterna. Devolve o estado de depois.
pub fn caffeine(ctx: &Ctx, sw: Switch) -> Result<bool> {
    let on = CAFFEINE.running(ctx);
    let next = sw.resolve(on);
    match (on, next) {
        (false, true) => {
            CAFFEINE.start(ctx, "systemd-inhibit", &caffeine_args())?;
            notify("café ligado", "a tela não apaga e o sistema não suspende");
        }
        (true, false) => {
            CAFFEINE.stop(ctx, "TERM")?;
            notify("café desligado", "a tela volta a apagar sozinha");
        }
        _ => {}
    }
    Ok(next)
}

// ── modo noturno ──────────────────────────────────────────────────────────

pub const NIGHT_TEMP: u32 = 3400;

pub fn night(ctx: &Ctx, sw: Switch, temp: Option<u32>) -> Result<bool> {
    let temp = temp.unwrap_or(NIGHT_TEMP);
    if !(1000..=10000).contains(&temp) {
        bail!("temperatura {temp}K fora do intervalo (1000 a 10000)");
    }
    let on = NIGHT.running(ctx);
    let next = sw.resolve(on);
    match (on, next) {
        (false, true) => {
            NIGHT.start(ctx, "hyprsunset", &["-t".into(), temp.to_string()])?;
            notify("modo noturno", &format!("tela mais quente ({temp}K)"));
        }
        (true, false) => {
            NIGHT.stop(ctx, "TERM")?;
        }
        _ => {}
    }
    Ok(next)
}

// ── não perturbe ──────────────────────────────────────────────────────────

fn dnd_file(ctx: &Ctx) -> PathBuf {
    ctx.paths.state.join("dnd")
}

/// A shell observa este arquivo: `1` silencia as notificações (as urgentes continuam passando).
pub fn dnd_on(ctx: &Ctx) -> bool {
    fs::read_to_string(dnd_file(ctx)).is_ok_and(|s| s.trim() == "1")
}

pub fn dnd(ctx: &Ctx, sw: Switch) -> Result<bool> {
    let on = dnd_on(ctx);
    let next = sw.resolve(on);
    if next && !on {
        // avisa antes de calar: depois não apareceria
        notify("não perturbe ligado", "as notificações ficam em silêncio, só as urgentes passam");
    }
    clios_core::fsutil::write_atomic(&dnd_file(ctx), if next { b"1\n" } else { b"0\n" })?;
    if !next && on {
        notify("não perturbe desligado", "as notificações voltam");
    }
    Ok(next)
}

/// "ligado" ou "desligado", para as mensagens.
pub fn state_word(on: bool) -> &'static str {
    if on { "ligado" } else { "desligado" }
}

// ── gravação de tela ──────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum RecWhat {
    /// Escolhe uma região (liga) ou para (se já está gravando).
    Toggle,
    Region,
    Screen,
    Stop,
}

fn videos_dir() -> PathBuf {
    std::env::var_os("XDG_VIDEOS_DIR")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join("Videos")))
        .unwrap_or_else(|| PathBuf::from("."))
}

fn timestamp() -> Result<String> {
    let out = Command::new("date").arg("+%Y%m%d-%H%M%S").output().context("rodando date")?;
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

/// Os argumentos do `wf-recorder`.
pub fn rec_args(geometry: Option<&str>, file: &str, audio: bool) -> Vec<String> {
    let mut a = Vec::new();
    if let Some(g) = geometry {
        a.extend(["-g".to_string(), g.to_string()]);
    }
    if audio {
        a.push("-a".into());
    }
    a.extend(["-f".to_string(), file.to_string()]);
    a
}

pub fn rec(ctx: &Ctx, what: RecWhat, audio: bool) -> Result<()> {
    let path_file = ctx.paths.state.join("run/rec.path");
    let running = REC.running(ctx);
    let stop = what == RecWhat::Stop || (what == RecWhat::Toggle && running);
    if stop {
        if !running {
            println!("não há gravação em andamento");
            return Ok(());
        }
        // SIGINT: o wf-recorder fecha o arquivo direito.
        REC.stop(ctx, "INT")?;
        let file = fs::read_to_string(&path_file).unwrap_or_default();
        let file = file.trim();
        notify("gravação salva", file);
        println!("{file}");
        return Ok(());
    }
    if running {
        bail!("já está gravando (clios rec stop para parar)");
    }

    let geometry = if what == RecWhat::Screen {
        None
    } else {
        let accent = ctx.theme()?.c.accent.hex();
        let out = Command::new("slurp")
            .args(["-b", "#00000099", "-c", &format!("{accent}ff"), "-s", "#00000000", "-w", "1"])
            .stderr(Stdio::null())
            .output()
            .context("slurp não encontrado (pacman -S slurp)")?;
        if !out.status.success() {
            return Ok(()); // cancelou com Esc
        }
        Some(String::from_utf8_lossy(&out.stdout).trim().to_string())
    };

    let dir = videos_dir();
    fs::create_dir_all(&dir).with_context(|| format!("criando {}", dir.display()))?;
    let file = dir.join(format!("clios-{}.mp4", timestamp()?));
    let args = rec_args(geometry.as_deref(), &file.display().to_string(), audio);
    REC.start(ctx, "wf-recorder", &args)?;
    clios_core::fsutil::write_atomic(&path_file, file.display().to_string().as_bytes())?;
    notify("gravando", "rode clios rec de novo (ou clique em gravando, na barra) para parar");
    println!("gravando em {}", file.display());
    Ok(())
}

// ── conta-gotas ───────────────────────────────────────────────────────────

pub fn pick() -> Result<()> {
    let out = Command::new("hyprpicker")
        .args(["-a", "-f", "hex"])
        .stderr(Stdio::null())
        .output()
        .context("hyprpicker não encontrado (paru -S hyprpicker)")?;
    let color = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if color.is_empty() {
        return Ok(()); // cancelou
    }
    notify("cor copiada", &color);
    println!("{color}");
    Ok(())
}

// ── energia ───────────────────────────────────────────────────────────────

pub const PROFILES: [&str; 3] = ["power-saver", "balanced", "performance"];

pub fn next_profile(current: &str) -> &'static str {
    let i = PROFILES.iter().position(|p| *p == current).unwrap_or(1);
    PROFILES[(i + 1) % PROFILES.len()]
}

pub fn power(profile: Option<&str>) -> Result<()> {
    let ctl = |args: &[&str]| -> Result<String> {
        let out = Command::new("powerprofilesctl")
            .args(args)
            .output()
            .context("powerprofilesctl não encontrado (pacman -S power-profiles-daemon)")?;
        if !out.status.success() {
            bail!("{}", String::from_utf8_lossy(&out.stderr).trim());
        }
        Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
    };
    let current = ctl(&["get"])?;
    let Some(want) = profile else {
        // sem argumento: passa para o próximo
        let next = next_profile(&current);
        ctl(&["set", next])?;
        notify("energia", next);
        println!("{next}");
        return Ok(());
    };
    if !PROFILES.contains(&want) {
        bail!("perfil {want:?} não existe. Use: {}", PROFILES.join(", "));
    }
    ctl(&["set", want])?;
    notify("energia", want);
    println!("{want}");
    Ok(())
}

// ── resumo para a barra ───────────────────────────────────────────────────

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct All {
    pub net: Net,
    pub caffeine: bool,
    pub night: bool,
    pub dnd: bool,
    pub rec: bool,
}

pub fn all(ctx: &Ctx) -> All {
    All {
        net: status::net(),
        caffeine: CAFFEINE.running(ctx),
        night: NIGHT.running(ctx),
        dnd: dnd_on(ctx),
        rec: REC.running(ctx),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn ctx(name: &str) -> Ctx {
        let home = std::env::temp_dir().join(format!("clios-tg-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&home);
        fs::create_dir_all(&home).unwrap();
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        Ctx::load(Some(&root), Some(&home)).unwrap()
    }

    #[test]
    fn switch_resolution() {
        assert!(Switch::On.resolve(false) && Switch::On.resolve(true));
        assert!(!Switch::Off.resolve(false) && !Switch::Off.resolve(true));
        assert!(Switch::Toggle.resolve(false));
        assert!(!Switch::Toggle.resolve(true));
    }

    #[test]
    fn caffeine_blocks_idle_and_sleep_until_killed() {
        let a = caffeine_args();
        assert!(a.contains(&"--what=idle:sleep".to_string()));
        assert!(a.ends_with(&["sleep".to_string(), "infinity".to_string()]));
    }

    #[test]
    fn recorder_args_with_and_without_region_and_audio() {
        assert_eq!(rec_args(None, "a.mp4", false), ["-f", "a.mp4"]);
        assert_eq!(rec_args(Some("10,20 300x200"), "a.mp4", true), ["-g", "10,20 300x200", "-a", "-f", "a.mp4"]);
    }

    #[test]
    fn power_profiles_cycle_and_wrap() {
        assert_eq!(next_profile("power-saver"), "balanced");
        assert_eq!(next_profile("balanced"), "performance");
        assert_eq!(next_profile("performance"), "power-saver");
        assert_eq!(next_profile("desconhecido"), "performance", "o desconhecido conta como equilibrado");
    }

    #[test]
    fn dnd_state_lives_in_a_file_the_shell_can_watch() {
        let c = ctx("dnd");
        assert!(!dnd_on(&c));
        assert!(dnd(&c, Switch::On).unwrap());
        assert!(dnd_on(&c));
        assert_eq!(fs::read_to_string(dnd_file(&c)).unwrap(), "1\n");
        assert!(!dnd(&c, Switch::Toggle).unwrap());
        assert!(!dnd_on(&c));
        assert!(!dnd(&c, Switch::Off).unwrap());
        assert!(!dnd_on(&c));
    }

    #[test]
    fn night_rejects_absurd_temperatures() {
        let c = ctx("night");
        assert!(night(&c, Switch::On, Some(50)).unwrap_err().to_string().contains("fora do intervalo"));
        assert!(night(&c, Switch::On, Some(99999)).is_err());
    }

    #[test]
    fn status_all_serializes_for_the_bar() {
        let c = ctx("all");
        dnd(&c, Switch::On).unwrap();
        let a = all(&c);
        assert!(a.dnd && !a.caffeine && !a.night && !a.rec);
        let j = serde_json::to_value(&a).unwrap();
        assert_eq!(j["dnd"], true);
        assert!(j["net"]["kind"].is_string());
    }
}
