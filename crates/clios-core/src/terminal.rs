//! Recolorir terminais abertos, sem reiniciar nada.
//!
//! Terminais aceitam OSC 4 (paleta), 10 (texto), 11 (fundo), 12 (cursor), 17/19 (seleção).
//! Escrever essa sequência no pty de cada terminal muda as cores na hora. É a mesma
//! técnica do pywal; aqui vem dos tokens.

use std::fmt::Write;

use crate::color::Rgb;
use crate::theme::Theme;

fn osc_rgb(c: Rgb) -> String {
    format!("rgb:{:02x}/{:02x}/{:02x}", c.r, c.g, c.b)
}

/// A sequência completa que deixa um terminal com a cara do tema.
/// Usa ST (`ESC \`) como terminador, que todo terminal moderno entende.
pub fn recolor_sequence(theme: &Theme) -> String {
    let c = &theme.c;
    let mut s = String::new();
    for (i, color) in c.ansi.iter().enumerate() {
        let _ = write!(s, "\x1b]4;{i};{}\x1b\\", osc_rgb(*color));
    }
    let _ = write!(s, "\x1b]10;{}\x1b\\", osc_rgb(c.fg));
    let _ = write!(s, "\x1b]11;{}\x1b\\", osc_rgb(c.bg));
    let _ = write!(s, "\x1b]12;{}\x1b\\", osc_rgb(c.accent));
    let _ = write!(s, "\x1b]17;{}\x1b\\", osc_rgb(c.accent_soft));
    let _ = write!(s, "\x1b]19;{}\x1b\\", osc_rgb(c.fg));
    s
}

/// Parâmetros `vt.default_*` do kernel, para o console de texto (boot, TTY) usar a mesma paleta.
/// Devolve algo como `vt.default_red=0,255,... vt.default_grn=... vt.default_blu=...`.
///
/// O console usa a cor 0 como fundo, então ela vira o fundo do tema (preto puro) em vez do
/// "preto ANSI" (#151515). Passe um tema escuro: o console não tem como inverter para claro.
pub fn kernel_vt_params(theme: &Theme) -> String {
    let mut palette = theme.c.ansi.clone();
    palette[0] = theme.c.bg;
    // O console do Linux usa a mesma ordem do ANSI: 0 preto, 1 vermelho, 2 verde, 3 amarelo...
    // e 8..=15 os brilhantes. São 16 valores por canal.
    let list = |f: fn(&Rgb) -> u8| -> String { palette.iter().map(|c| f(c).to_string()).collect::<Vec<_>>().join(",") };
    format!("vt.default_red={} vt.default_grn={} vt.default_blu={}", list(|c| c.r), list(|c| c.g), list(|c| c.b))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tokens::{Mode, MotionLevel, Tokens};

    fn theme() -> Theme {
        Theme::resolve(&Tokens::builtin(), Mode::Dark, "ember", MotionLevel::Full).unwrap()
    }

    #[test]
    fn sequence_sets_palette_fg_bg_and_cursor() {
        let s = recolor_sequence(&theme());
        assert!(s.contains("\x1b]4;1;rgb:ff/61/66\x1b\\"), "vermelho ANSI");
        assert!(s.contains("\x1b]4;15;rgb:f5/f5/f5\x1b\\"), "branco brilhante");
        assert!(s.contains("\x1b]11;rgb:00/00/00\x1b\\"), "fundo");
        assert!(s.contains("\x1b]12;rgb:ff/5a/1f\x1b\\"), "cursor no acento");
        assert_eq!(s.matches("\x1b]4;").count(), 16);
    }

    #[test]
    fn kernel_console_background_is_pure_theme_bg_not_ansi_black() {
        let p = kernel_vt_params(&theme());
        // índice 0 de cada canal = 0 (preto puro), não 0x15 (#151515)
        assert!(p.starts_with("vt.default_red=0,"), "{p}");
        assert!(p.contains(" vt.default_grn=0,") && p.contains(" vt.default_blu=0,"), "{p}");
    }

    #[test]
    fn kernel_params_have_sixteen_values_per_channel() {
        let p = kernel_vt_params(&theme());
        for key in ["vt.default_red=", "vt.default_grn=", "vt.default_blu="] {
            let part = p.split(' ').find(|x| x.starts_with(key)).unwrap();
            assert_eq!(part[key.len()..].split(',').count(), 16, "{key}");
        }
    }
}
