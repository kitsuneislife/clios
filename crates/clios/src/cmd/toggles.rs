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

/// O modo noturno tem um estado a mais: o agendado.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum NightSwitch {
    On,
    Off,
    Toggle,
    /// Todo dia, num horário (`clios night auto 20:30-06:45`, ou `off`).
    Auto,
}

impl NightSwitch {
    pub fn plain(self) -> Switch {
        match self {
            NightSwitch::On => Switch::On,
            NightSwitch::Off => Switch::Off,
            _ => Switch::Toggle,
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
    let on = night_on(ctx);
    let next = sw.resolve(on);
    // mexer à mão vale até o próximo login: o agendado sai de cena
    let was_auto = auto_marker(ctx).exists();
    let _ = fs::remove_file(auto_marker(ctx));
    if was_auto && NIGHT.running(ctx) {
        NIGHT.stop(ctx, "TERM")?;
    }
    let on = on && !was_auto;
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

/// O horário do noturno agendado, em minutos do dia. Pode passar da meia-noite (`20:30-06:45`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Window {
    pub start: u32,
    pub end: u32,
}

fn hhmm(s: &str) -> Option<u32> {
    let (h, m) = s.trim().split_once(':')?;
    let (h, m): (u32, u32) = (h.parse().ok()?, m.parse().ok()?);
    (h < 24 && m < 60).then_some(h * 60 + m)
}

fn fmt_hhmm(m: u32) -> String {
    format!("{:02}:{:02}", m / 60, m % 60)
}

impl Window {
    pub fn parse(s: &str) -> Option<Self> {
        let (a, b) = s.split_once('-')?;
        let w = Window { start: hhmm(a)?, end: hhmm(b)? };
        (w.start != w.end).then_some(w)
    }

    pub fn contains(self, minute: u32) -> bool {
        if self.start < self.end {
            (self.start..self.end).contains(&minute)
        } else {
            minute >= self.start || minute < self.end
        }
    }

    pub fn label(self) -> String {
        format!("{}-{}", fmt_hhmm(self.start), fmt_hhmm(self.end))
    }

    /// A config do hyprsunset: um perfil que esquenta no começo e outro que volta ao normal no fim.
    pub fn hyprsunset_conf(self, temp: u32) -> String {
        format!(
            "# Gerado por `clios night auto {}`. O hyprsunset troca de perfil sozinho no horário.\n\n\
             profile {{\n    time = {}\n    identity = true\n}}\n\n\
             profile {{\n    time = {}\n    temperature = {temp}\n}}\n",
            self.label(),
            fmt_hhmm(self.end),
            fmt_hhmm(self.start)
        )
    }
}

fn auto_marker(ctx: &Ctx) -> PathBuf {
    ctx.paths.state.join("night-auto")
}

fn local_minute() -> Option<u32> {
    let out = Command::new("date").arg("+%H:%M").output().ok()?;
    hhmm(&String::from_utf8_lossy(&out.stdout))
}

/// A tela está mais quente agora? No agendado, o hyprsunset roda o dia todo, mas só esquenta no horário.
pub fn night_on(ctx: &Ctx) -> bool {
    if !NIGHT.running(ctx) {
        return false;
    }
    if !auto_marker(ctx).exists() {
        return true;
    }
    match (Window::parse(&ctx.state.night), local_minute()) {
        (Some(w), Some(m)) => w.contains(m),
        _ => true,
    }
}

fn start_auto(ctx: &Ctx, w: Window) -> Result<()> {
    let conf = ctx.paths.config.join("hypr/hyprsunset.conf");
    clios_core::fsutil::write_atomic(&conf, w.hyprsunset_conf(NIGHT_TEMP).as_bytes())?;
    if NIGHT.running(ctx) {
        NIGHT.stop(ctx, "TERM")?;
    }
    NIGHT.start(ctx, "hyprsunset", &[])?;
    clios_core::fsutil::write_atomic(&auto_marker(ctx), w.label().as_bytes())?;
    Ok(())
}

/// `clios night auto 20:30-06:45` agenda; `off` desfaz.
pub fn night_auto(ctx: &mut Ctx, spec: &str) -> Result<()> {
    if matches!(spec, "off" | "desligar") {
        ctx.state.night = "off".into();
        ctx.save_state()?;
        if auto_marker(ctx).exists() {
            let _ = fs::remove_file(auto_marker(ctx));
            if NIGHT.running(ctx) {
                NIGHT.stop(ctx, "TERM")?;
            }
        }
        println!("modo noturno só à mão (super + n)");
        return Ok(());
    }
    let Some(w) = Window::parse(spec) else { bail!("horário {spec:?} inválido: use início-fim, como 20:30-06:45") };
    ctx.state.night = w.label();
    ctx.save_state()?;
    if !ctx.sandboxed {
        start_auto(ctx, w)?;
    }
    println!("a tela esquenta às {} e volta ao normal às {}, todo dia", fmt_hhmm(w.start), fmt_hhmm(w.end));
    Ok(())
}

/// No login: liga o agendado, se houver.
pub fn night_login(ctx: &Ctx) -> Result<()> {
    match Window::parse(&ctx.state.night) {
        Some(w) => start_auto(ctx, w),
        None => Ok(()),
    }
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
    /// Segundos que faltam do bloco de foco (`clios focus`); 0 sem bloco.
    pub focus: u64,
    /// O kernel foi atualizado e falta reiniciar.
    pub reboot: bool,
}

pub fn all(ctx: &Ctx) -> All {
    All {
        net: status::net(),
        caffeine: CAFFEINE.running(ctx),
        night: night_on(ctx),
        dnd: dnd_on(ctx),
        rec: REC.running(ctx),
        focus: super::focus::remaining(ctx).unwrap_or(0),
        reboot: super::snap::reboot_pending(),
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
    fn night_windows_cross_midnight() {
        let w = Window::parse("20:30-06:45").unwrap();
        assert!(w.contains(21 * 60) && w.contains(3 * 60) && !w.contains(12 * 60) && !w.contains(6 * 60 + 45));
        let d = Window::parse("13:00-14:00").unwrap();
        assert!(d.contains(13 * 60 + 30) && !d.contains(14 * 60));
        assert_eq!(w.label(), "20:30-06:45");
        for bad in ["", "20:30", "25:00-06:00", "20:30-20:30", "8-9", "ab:cd-01:00"] {
            assert_eq!(Window::parse(bad), None, "{bad}");
        }
    }

    #[test]
    fn the_hyprsunset_config_has_a_warm_and_a_neutral_profile() {
        let c = Window::parse("21:00-07:30").unwrap().hyprsunset_conf(3400);
        assert!(c.contains("profile {\n    time = 07:30\n    identity = true\n}"), "{c}");
        assert!(c.contains("profile {\n    time = 21:00\n    temperature = 3400\n}"), "{c}");
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
        assert!(a.dnd && !a.caffeine && !a.night && !a.rec && a.focus == 0);
        let j = serde_json::to_value(&a).unwrap();
        assert_eq!(j["dnd"], true);
        assert!(j["net"]["kind"].is_string());
        assert!(j["reboot"].is_boolean());
    }
}
