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
