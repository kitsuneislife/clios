//! Saída de terminal para os comandos não interativos.

use std::io::IsTerminal;

use clios_core::Rgb;

pub fn color_enabled() -> bool {
    std::io::stdout().is_terminal() && std::env::var_os("NO_COLOR").is_none()
}

/// Bloco colorido de duas células, ou `##` se o terminal não quer cor.
pub fn swatch(c: Rgb) -> String {
    if color_enabled() { format!("\x1b[48;2;{};{};{}m  \x1b[0m", c.r, c.g, c.b) } else { "##".into() }
}

pub fn dim(s: &str) -> String {
    if color_enabled() { format!("\x1b[2m{s}\x1b[0m") } else { s.to_string() }
}

pub fn bold(s: &str) -> String {
    if color_enabled() { format!("\x1b[1m{s}\x1b[0m") } else { s.to_string() }
}

/// Um buffer do ratatui como texto ANSI de 24 bits (para `--snapshot`, docs e capturas).
pub fn buffer_ansi(buf: &ratatui::buffer::Buffer) -> String {
    use ratatui::style::{Color, Modifier};
    use std::fmt::Write as _;

    let rgb = |c: Color| match c {
        Color::Rgb(r, g, b) => (r, g, b),
        _ => (128, 128, 128),
    };
    let mut out = String::new();
    for y in 0..buf.area.height {
        for x in 0..buf.area.width {
            let c = &buf[(x, y)];
            let (fr, fg, fb) = rgb(c.fg);
            let (br, bg, bb) = rgb(c.bg);
            let bold = if c.modifier.contains(Modifier::BOLD) { "0;1;" } else { "0;" };
            let _ = write!(out, "\x1b[{bold}38;2;{fr};{fg};{fb};48;2;{br};{bg};{bb}m{}", c.symbol());
        }
        out.push_str("\x1b[0m\n");
    }
    out
}
