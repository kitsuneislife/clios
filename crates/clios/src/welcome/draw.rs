//! Primitivas de desenho do guia: texto, pílulas, teclas e a entrada escalonada.

use clios_core::{Rgb, Theme};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

pub const PAD: u16 = 3;
/// Tampas redondas das pílulas: glifos powerline da Nerd Font (o foot usa a GeistMono Nerd Font).
pub const CAP_L: &str = "\u{e0b6}";
pub const CAP_R: &str = "\u{e0b4}";

pub fn color(c: Rgb) -> Color {
    Color::Rgb(c.r, c.g, c.b)
}

/// As cores que o guia usa, tiradas do tema.
#[derive(Debug, Clone, Copy)]
pub struct Pen {
    pub bg: Rgb,
    pub surface: Rgb,
    pub raised: Rgb,
    pub line: Rgb,
    pub mute: Rgb,
    pub dim: Rgb,
    pub fg: Rgb,
    pub accent: Rgb,
    pub green: Rgb,
    pub yellow: Rgb,
    pub red: Rgb,
}

impl Pen {
    pub fn of(t: &Theme) -> Self {
        let c = &t.c;
        Self {
            bg: c.bg,
            surface: c.surface,
            raised: c.raised,
            line: c.line,
            mute: c.mute,
            dim: c.dim,
            fg: c.fg,
            accent: c.accent,
            green: c.green,
            yellow: c.yellow,
            red: c.red,
        }
    }
}

pub fn text_w(s: &str) -> u16 {
    UnicodeWidthStr::width(s).min(u16::MAX as usize) as u16
}

/// Escreve `s` a partir de (x, y), cortando na borda do buffer. Devolve a coluna seguinte.
pub fn put(buf: &mut Buffer, x: u16, y: u16, s: &str, fg: Rgb, bg: Rgb, bold: bool) -> u16 {
    let area = buf.area;
    let mut x = x;
    if y >= area.bottom() {
        return x;
    }
    for ch in s.chars() {
        let w = ch.width().unwrap_or(0) as u16;
        if x + w > area.right() {
            break;
        }
        if x >= area.left() {
            let cell = &mut buf[(x, y)];
            cell.set_char(ch).set_fg(color(fg)).set_bg(color(bg));
            if bold {
                cell.modifier |= Modifier::BOLD;
            }
        }
        x += w.max(1);
    }
    x
}

pub fn fill(buf: &mut Buffer, r: Rect, bg: Rgb) {
    let r = r.intersection(buf.area);
    for y in r.top()..r.bottom() {
        for x in r.left()..r.right() {
            buf[(x, y)].set_char(' ').set_bg(color(bg));
        }
    }
}

/// Uma barra de fundo com as pontas redondas, de `x` a `x + w` (as tampas ficam em `x - 1` e `x + w`).
pub fn pill(buf: &mut Buffer, x: u16, y: u16, w: u16, fill_color: Rgb, page_bg: Rgb) {
    for i in 0..w {
        put(buf, x + i, y, " ", fill_color, fill_color, false);
    }
    put(buf, x.saturating_sub(1), y, CAP_L, fill_color, page_bg, false);
    put(buf, x + w, y, CAP_R, fill_color, page_bg, false);
}

/// O formato dos atalhos: cada palavra vira uma tecla; o `+` só separa.
pub fn key_tokens(keys: &str) -> Vec<&str> {
    keys.split_whitespace().filter(|t| *t != "+").collect()
}

/// Isto parece um atalho de teclado (e não um comando ou um caminho)?
pub fn is_shortcut(keys: &str) -> bool {
    const NAMES: [&str; 9] = ["super", "shift", "ctrl", "alt", "print", "espaço", "enter", "esc", "tab"];
    let toks = key_tokens(keys);
    !toks.is_empty()
        && toks.iter().all(|t| {
            NAMES.contains(t)
                || t.chars().count() == 1
                || *t == "…"
                || (t.starts_with('F') && t[1..].parse::<u8>().is_ok())
        })
}

pub fn keycaps_width(keys: &str) -> u16 {
    let toks = key_tokens(keys);
    toks.iter().map(|t| text_w(t) + 4).sum::<u16>() + toks.len().saturating_sub(1) as u16
}

/// Desenha as teclas como pílulas pequenas. Devolve a coluna seguinte.
pub fn keycaps(buf: &mut Buffer, x: u16, y: u16, keys: &str, pen: &Pen, page_bg: Rgb) -> u16 {
    let mut x = x;
    for (i, t) in key_tokens(keys).iter().enumerate() {
        if i > 0 {
            x += 1;
        }
        let w = text_w(t) + 2;
        put(buf, x, y, CAP_L, pen.raised, page_bg, false);
        put(buf, x + 1, y, &format!(" {t} "), pen.fg, pen.raised, false);
        put(buf, x + 1 + w, y, CAP_R, pen.raised, page_bg, false);
        x += w + 2;
    }
    x
}

pub fn code_width(cmd: &str) -> u16 {
    text_w(cmd) + 6
}

/// Um comando, numa pílula de fundo recuado e texto no acento. Ocupa `code_width` colunas a partir de `x`.
pub fn code(buf: &mut Buffer, x: u16, y: u16, cmd: &str, pen: &Pen, page_bg: Rgb) -> u16 {
    let w = text_w(cmd) + 4;
    pill(buf, x + 1, y, w, pen.surface, page_bg);
    put(buf, x + 3, y, cmd, pen.accent, pen.surface, false);
    x + w + 2
}

/// Quebra `text` em linhas de até `width` colunas, sem cortar palavras.
pub fn wrap(text: &str, width: u16) -> Vec<String> {
    let width = usize::from(width.max(1));
    let mut lines = Vec::new();
    let mut cur = String::new();
    for word in text.split_whitespace() {
        let need = if cur.is_empty() { text_w(word) } else { text_w(&cur) + 1 + text_w(word) } as usize;
        if need > width && !cur.is_empty() {
            lines.push(std::mem::take(&mut cur));
        }
        if !cur.is_empty() {
            cur.push(' ');
        }
        cur.push_str(word);
    }
    if !cur.is_empty() {
        lines.push(cur);
    }
    lines
}

/// Corta em `width` colunas, com reticências se precisou.
pub fn truncate(s: &str, width: u16) -> String {
    if text_w(s) <= width {
        return s.to_string();
    }
    let mut out = String::new();
    let mut w = 0;
    for ch in s.chars() {
        let cw = ch.width().unwrap_or(0) as u16;
        if w + cw + 1 > width {
            break;
        }
        out.push(ch);
        w += cw;
    }
    out.push('…');
    out
}

pub fn rule(buf: &mut Buffer, x0: u16, x1: u16, y: u16, pen: &Pen) {
    for x in x0..x1 {
        put(buf, x, y, "─", pen.line, pen.bg, false);
    }
}

/// A primeira linha visível de uma lista de `rows` linhas, para a seleção ficar sempre à vista.
pub fn first_visible(sel: usize, rows: u16) -> usize {
    (sel + 1).saturating_sub(usize::from(rows))
}

/// Uma linha de lista: pílula quando selecionada, com o `❯` do acento.
pub fn list_row(buf: &mut Buffer, pen: &Pen, at: (u16, u16, u16), selected: bool, label: &str, right: &str) {
    let (x, y, w) = at;
    let bg = if selected { pen.raised } else { pen.bg };
    if selected {
        pill(buf, x, y, w, pen.raised, pen.bg);
        put(buf, x, y, "❯", pen.accent, bg, true);
    }
    let label = truncate(label, w.saturating_sub(4 + text_w(right)));
    put(buf, x + 2, y, &label, if selected { pen.fg } else { pen.dim }, bg, selected);
    if !right.is_empty() {
        put(buf, x + w.saturating_sub(text_w(right) + 1), y, right, pen.mute, bg, false);
    }
}

/// A entrada: cada linha de `area` aparece em fade, uma depois da outra. `elapsed_ms` conta desde a troca de página.
/// O fade vai do fundo da página até a cor final, na curva de chegada do tema.
pub fn reveal(
    buf: &mut Buffer,
    area: Rect,
    bg: Rgb,
    elapsed_ms: u64,
    stagger_ms: u64,
    fade_ms: u64,
    ease: &dyn Fn(f64) -> f64,
) {
    let area = area.intersection(buf.area);
    for (i, y) in (area.top()..area.bottom()).enumerate() {
        let t = elapsed_ms.saturating_sub(stagger_ms * i as u64) as f64 / fade_ms.max(1) as f64;
        if t >= 1.0 {
            continue;
        }
        let k = ease(t.max(0.0));
        for x in area.left()..area.right() {
            let cell = &mut buf[(x, y)];
            let mix = |c: Color| match c {
                Color::Rgb(r, g, b) => {
                    let m = bg.blend(Rgb { r, g, b }, k);
                    Color::Rgb(m.r, m.g, m.b)
                }
                other => other,
            };
            cell.set_fg(mix(cell.fg));
            cell.set_bg(mix(cell.bg));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wrapping_never_cuts_words_or_exceeds_the_width() {
        let lines = wrap("um dois três quatro cinco seis sete oito", 14);
        assert!(lines.iter().all(|l| text_w(l) <= 14), "{lines:?}");
        assert_eq!(lines.join(" "), "um dois três quatro cinco seis sete oito");
        assert_eq!(wrap("palavra-gigante-demais", 5), ["palavra-gigante-demais"]);
        assert!(wrap("", 10).is_empty());
    }

    #[test]
    fn truncation_adds_an_ellipsis_within_width() {
        assert_eq!(truncate("curto", 10), "curto");
        let t = truncate("uma frase bem comprida", 10);
        assert!(text_w(&t) <= 10 && t.ends_with('…'), "{t}");
    }

    #[test]
    fn shortcuts_are_told_apart_from_commands() {
        assert!(is_shortcut("super + espaço"));
        assert!(is_shortcut("super + shift + F4"));
        assert!(is_shortcut("super + 1 … 0"));
        assert!(is_shortcut("print"));
        assert!(!is_shortcut("clios apps list"));
        assert!(!is_shortcut("~/.config/clios/hub.toml"));
        assert!(!is_shortcut("super + espaço, depois +"));
    }

    #[test]
    fn keycap_width_matches_what_is_drawn() {
        use clios_core::{Mode, MotionLevel, Tokens};
        let theme = Theme::resolve(&Tokens::builtin(), Mode::Dark, "ember", MotionLevel::Full).unwrap();
        let pen = Pen::of(&theme);
        for keys in ["super + espaço", "print", "super + shift + F4"] {
            let mut buf = Buffer::empty(Rect::new(0, 0, 60, 1));
            let end = keycaps(&mut buf, 2, 0, keys, &pen, pen.bg);
            assert_eq!(end - 2, keycaps_width(keys), "{keys}");
        }
    }

    #[test]
    fn reveal_hides_late_rows_and_leaves_finished_ones_alone() {
        let bg = Rgb { r: 0, g: 0, b: 0 };
        let fgc = Rgb { r: 200, g: 200, b: 200 };
        let mut buf = Buffer::empty(Rect::new(0, 0, 4, 3));
        for y in 0..3 {
            put(&mut buf, 0, y, "x", fgc, bg, false);
        }
        let area = buf.area;
        reveal(&mut buf, area, bg, 100, 100, 150, &|t| t);
        assert_eq!(buf[(0, 0)].fg, color(bg.blend(fgc, 100.0 / 150.0)), "a primeira linha está no meio do fade");
        assert_eq!(buf[(0, 1)].fg, color(bg), "a segunda ainda não começou");
        assert_eq!(buf[(0, 2)].fg, color(bg), "nem a terceira");
        let mut done = Buffer::empty(Rect::new(0, 0, 4, 1));
        put(&mut done, 0, 0, "x", fgc, bg, false);
        let area = done.area;
        reveal(&mut done, area, bg, 1000, 100, 150, &|t| t);
        assert_eq!(done[(0, 0)].fg, color(fgc));
    }
}
