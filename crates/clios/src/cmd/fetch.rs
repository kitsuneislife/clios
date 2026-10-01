//! `clios fetch`: o resumo do sistema, com a marca ao lado (o neofetch do CLIOS).

use anyhow::Result;
use clios_core::Theme;

use crate::art;
use crate::ctx::Ctx;
use crate::sysinfo::{self, Info};
use crate::ui;

/// As linhas `(rótulo, valor)`, na ordem de exibição.
pub fn rows(info: &Info, theme: &Theme, wallpaper: &str, motion: &str) -> Vec<(&'static str, String)> {
    let mut v = vec![("os", info.os.clone()), ("kernel", info.kernel.clone())];
    if let Some(u) = info.uptime {
        v.push(("uptime", sysinfo::fmt_duration(u)));
    }
    if !info.shell.is_empty() {
        v.push(("shell", info.shell.clone()));
    }
    if !info.wm.is_empty() {
        v.push(("wm", info.wm.clone()));
    }
    if let Some(cpu) = &info.cpu {
        v.push(("cpu", cpu.clone()));
    }
    if let Some((used, total)) = info.mem {
        v.push(("memória", format!("{} / {}", sysinfo::fmt_bytes(used), sysinfo::fmt_bytes(total))));
    }
    if let Some((used, total)) = info.disk {
        v.push(("disco", format!("{} / {}", sysinfo::fmt_bytes(used), sysinfo::fmt_bytes(total))));
    }
    if let Some(n) = info.packages {
        v.push(("pacotes", n.to_string()));
    }
    v.push(("tema", format!("{} · {} · movimento {motion}", theme.accent_name, theme.mode)));
    v.push(("fundo", wallpaper.to_string()));
    v
}

pub fn run(ctx: &Ctx) -> Result<()> {
    let info = sysinfo::gather();
    let theme = ctx.theme()?;
    let rows = rows(&info, &theme, &ctx.state.wallpaper, &ctx.state.motion.to_string());
    let who = if info.user.is_empty() { info.host.clone() } else { format!("{}@{}", info.user, info.host) };

    if !ui::color_enabled() {
        println!("{who}");
        for (k, v) in &rows {
            println!("{k}: {v}");
        }
        return Ok(());
    }

    const ART_ROWS: u32 = 10;
    let art = art::mark_ansi(&theme, ART_ROWS, true);
    let pad = " ".repeat((ART_ROWS * 2) as usize);
    let mut text: Vec<String> = Vec::new();
    text.push(ui::bold(&who));
    text.push(ui::dim(&"─".repeat(who.chars().count())));
    let key_w = rows.iter().map(|(k, _)| k.chars().count()).max().unwrap_or(0);
    for (k, v) in &rows {
        let c = theme.c.accent;
        text.push(format!("\x1b[38;2;{};{};{}m{k:<key_w$}\x1b[0m  {v}", c.r, c.g, c.b));
    }
    // a paleta: o acento e as oito cores ANSI
    let mut sw = ui::swatch(theme.c.accent);
    sw.push(' ');
    for c in &theme.c.ansi[..8] {
        sw.push_str(&ui::swatch(*c));
    }
    text.push(String::new());
    text.push(sw);

    println!();
    for i in 0..art.len().max(text.len()) {
        let left = art.get(i).map_or(pad.as_str(), String::as_str);
        println!("  {left}   {}", text.get(i).map_or("", String::as_str));
    }
    println!();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use clios_core::{Mode, MotionLevel, Tokens};

    fn theme() -> Theme {
        Theme::resolve(&Tokens::builtin(), Mode::Dark, "mint", MotionLevel::Full).unwrap()
    }

    #[test]
    fn rows_skip_what_is_unknown_and_always_show_theme_and_wallpaper() {
        let info = Info { os: "Arch Linux".into(), kernel: "6.18.1".into(), ..Info::default() };
        let r = rows(&info, &theme(), "grade", "full");
        let keys: Vec<&str> = r.iter().map(|(k, _)| *k).collect();
        assert_eq!(keys, ["os", "kernel", "tema", "fundo"]);
        assert_eq!(r[2].1, "mint · dark · movimento full");
    }

    #[test]
    fn rows_format_memory_and_disk() {
        let info = Info {
            os: "x".into(),
            kernel: "y".into(),
            mem: Some((3 * 1024 * 1024 * 1024, 16 * 1024 * 1024 * 1024)),
            disk: Some((50 * 1024 * 1024 * 1024, 500 * 1024 * 1024 * 1024)),
            packages: Some(812),
            ..Info::default()
        };
        let r = rows(&info, &theme(), "grade", "off");
        assert!(r.contains(&("memória", "3.0 GiB / 16.0 GiB".to_string())));
        assert!(r.contains(&("disco", "50.0 GiB / 500 GiB".to_string())));
        assert!(r.contains(&("pacotes", "812".to_string())));
    }
}
