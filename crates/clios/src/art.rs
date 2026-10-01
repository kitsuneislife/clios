//! A marca desenhada em meio-bloco (`▀`): cada célula carrega dois pixels, o de cima na cor do texto
//! e o de baixo no fundo. Vem do mesmo contorno que gera o SVG e o papel de parede.

use clios_core::wallpaper::mark_coverage;
use clios_core::{Rgb, Theme};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cell {
    /// Pixel de cima.
    pub top: Rgb,
    /// Pixel de baixo.
    pub bottom: Rgb,
}

/// A marca em `rows` linhas de terminal (e `rows * 2` colunas, para ficar quadrada), sobre `backdrop`.
/// `cursor_on = false` apaga o cursor: é assim que ele pisca.
pub fn mark_cells(theme: &Theme, rows: u32, cursor_on: bool, backdrop: Rgb) -> Vec<Cell> {
    let (w, h) = (rows * 2, rows * 2);
    let cov = mark_coverage(w, h, cursor_on);
    let c = &theme.c;
    let px = |i: u32, j: u32| {
        let [body, cursor] = cov[(j * w + i) as usize];
        backdrop.blend(c.fg, f64::from(body)).blend(c.accent, f64::from(cursor))
    };
    let mut out = Vec::with_capacity((w * rows) as usize);
    for r in 0..rows {
        for i in 0..w {
            out.push(Cell { top: px(i, r * 2), bottom: px(i, r * 2 + 1) });
        }
    }
    out
}

/// A mesma marca como texto ANSI de 24 bits, uma `String` por linha (para o `clios fetch`).
pub fn mark_ansi(theme: &Theme, rows: u32, cursor_on: bool) -> Vec<String> {
    let cells = mark_cells(theme, rows, cursor_on, theme.c.bg);
    let w = (rows * 2) as usize;
    cells
        .chunks(w)
        .map(|line| {
            let mut s = String::new();
            for c in line {
                s.push_str(&format!(
                    "\x1b[38;2;{};{};{};48;2;{};{};{}m▀",
                    c.top.r, c.top.g, c.top.b, c.bottom.r, c.bottom.g, c.bottom.b
                ));
            }
            s.push_str("\x1b[0m");
            s
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use clios_core::{Mode, MotionLevel, Tokens};

    fn theme() -> Theme {
        Theme::resolve(&Tokens::builtin(), Mode::Dark, "ember", MotionLevel::Full).unwrap()
    }

    #[test]
    fn mark_is_square_and_has_body_and_accent() {
        let t = theme();
        let cells = mark_cells(&t, 8, true, t.c.bg);
        assert_eq!(cells.len(), 16 * 8);
        assert!(cells.iter().any(|c| c.top == t.c.fg || c.bottom == t.c.fg), "corpo na cor do texto");
        assert!(cells.iter().any(|c| c.top == t.c.accent || c.bottom == t.c.accent), "cursor no acento");
        assert!(cells.iter().any(|c| c.top == t.c.bg && c.bottom == t.c.bg), "a abertura é fundo puro");
    }

    #[test]
    fn blink_off_removes_every_accent_pixel() {
        let t = theme();
        let cells = mark_cells(&t, 8, false, t.c.bg);
        assert!(cells.iter().all(|c| c.top != t.c.accent && c.bottom != t.c.accent));
    }

    #[test]
    fn ansi_has_one_line_per_row() {
        let lines = mark_ansi(&theme(), 6, true);
        assert_eq!(lines.len(), 6);
        assert!(lines.iter().all(|l| l.contains('▀') && l.ends_with("\x1b[0m")));
    }
}
