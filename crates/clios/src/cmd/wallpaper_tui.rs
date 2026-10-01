//! O seletor de papel de parede: lista à esquerda, miniatura ao vivo à direita.
//!
//! A miniatura é o mesmo motor que gera o papel de parede de verdade, em meio-bloco (`▀`):
//! o que você vê é o que vai para a tela, nas cores do tema atual.

use std::collections::HashMap;
use std::io;
use std::time::Duration;

use anyhow::Result;
use clios_core::wallpaper;
use clios_core::{Rgb, Theme};
use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{
    self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind, KeyModifiers, MouseButton,
    MouseEventKind,
};
use ratatui::crossterm::execute;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style as Sty};

use super::wallpaper::{self as wp, Choice};
use crate::ctx::Ctx;

const PAD: u16 = 3;
const LIST_W: u16 = 24;
const CAP_L: &str = "\u{e0b6}";
const CAP_R: &str = "\u{e0b4}";

fn color(c: Rgb) -> Color {
    Color::Rgb(c.r, c.g, c.b)
}

pub struct Picker {
    pub choices: Vec<Choice>,
    pub sel: usize,
    /// Id do que está aplicado agora.
    pub current: String,
    pub theme: Theme,
    pub notice: String,
    thumbs: HashMap<(usize, u16, u16), Vec<Rgb>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Geometry {
    pub list_top: u16,
    pub list_rows: u16,
    /// Caixa da miniatura, com a borda.
    pub preview: Rect,
    pub footer_y: u16,
}

pub fn geometry(area: Rect) -> Geometry {
    let footer_y = area.height.saturating_sub(2);
    let list_top = 3;
    let preview_x = PAD + LIST_W + 3;
    let preview_w = area.width.saturating_sub(preview_x + PAD);
    // duas linhas embaixo da miniatura: nome e descrição
    let preview_h = footer_y.saturating_sub(list_top + 4);
    Geometry {
        list_top,
        list_rows: footer_y.saturating_sub(list_top + 1),
        preview: Rect::new(preview_x, list_top, preview_w, preview_h),
        footer_y,
    }
}

impl Picker {
    pub fn new(ctx: &Ctx, theme: Theme) -> Self {
        let choices = wp::all_choices(&ctx.paths);
        let current = ctx.state.wallpaper.clone();
        let sel = choices.iter().position(|c| c.id() == current).unwrap_or(0);
        Picker { choices, sel, current, theme, notice: String::new(), thumbs: HashMap::new() }
    }

    pub fn move_by(&mut self, delta: isize) {
        self.sel = (self.sel as isize + delta).rem_euclid(self.choices.len() as isize) as usize;
        self.notice.clear();
    }

    /// Garante a miniatura da seleção para o tamanho da caixa (só calcula uma vez por tamanho).
    pub fn prepare(&mut self, geo: &Geometry) {
        let (cols, rows) = (geo.preview.width.saturating_sub(2), geo.preview.height.saturating_sub(2));
        if cols == 0 || rows == 0 {
            return;
        }
        if let Choice::Style(s) = &self.choices[self.sel] {
            let key = (self.sel, cols, rows);
            let theme = &self.theme;
            self.thumbs
                .entry(key)
                .or_insert_with(|| wallpaper::thumbnail(*s, &theme.c, u32::from(cols), u32::from(rows)));
        }
    }
}

/// A primeira linha da lista que aparece, para a seleção ficar sempre à vista.
fn first_visible(sel: usize, rows: u16) -> usize {
    (sel + 1).saturating_sub(usize::from(rows))
}

fn put(buf: &mut Buffer, x: u16, y: u16, s: &str, fg: Color, bg: Color, bold: bool) -> u16 {
    use unicode_width::UnicodeWidthChar;
    let area = buf.area;
    let mut x = x;
    for ch in s.chars() {
        let w = ch.width().unwrap_or(0) as u16;
        if x + w > area.right() || y >= area.bottom() {
            break;
        }
        let cell = &mut buf[(x, y)];
        cell.set_char(ch).set_fg(fg).set_bg(bg);
        if bold {
            cell.modifier |= Modifier::BOLD;
        }
        x += w.max(1);
    }
    x
}

pub fn render(buf: &mut Buffer, p: &Picker) {
    let area = buf.area;
    let c = &p.theme.c;
    let (bg, fg, dim, mute, line, accent) =
        (color(c.bg), color(c.fg), color(c.dim), color(c.mute), color(c.line), color(c.accent));
    buf.set_style(area, Sty::default().bg(bg).fg(fg));
    let geo = geometry(area);

    put(buf, PAD, 1, "papel de parede", fg, bg, true);
    let right = format!("{} · {}", p.theme.accent_name, p.theme.mode);
    let rx = area.width.saturating_sub(PAD + right.chars().count() as u16);
    put(buf, rx, 1, &right, mute, bg, false);

    // lista
    let first = first_visible(p.sel, geo.list_rows);
    for (row, (i, ch)) in p.choices.iter().enumerate().skip(first).take(usize::from(geo.list_rows)).enumerate() {
        let y = geo.list_top + row as u16;
        let selected = i == p.sel;
        let applied = ch.id() == p.current;
        let (left, right_edge) = (PAD, PAD + LIST_W);
        let row_bg = if selected { color(c.bg.blend(c.raised, 1.0)) } else { bg };
        if selected {
            for x in left..right_edge {
                put(buf, x, y, " ", fg, row_bg, false);
            }
            put(buf, left - 1, y, CAP_L, row_bg, bg, false);
            put(buf, right_edge, y, CAP_R, row_bg, bg, false);
            put(buf, left, y, "❯", accent, row_bg, true);
        }
        let label = ch.label();
        let label_fg = if selected { fg } else { dim };
        put(buf, left + 2, y, &label, label_fg, row_bg, selected);
        if applied {
            put(buf, right_edge - 2, y, "●", accent, row_bg, false);
        }
    }

    // miniatura, numa caixa de cantos arredondados
    let b = geo.preview;
    if b.width >= 4 && b.height >= 4 {
        let (x0, y0, x1, y1) = (b.x, b.y, b.right() - 1, b.bottom() - 1);
        for x in x0 + 1..x1 {
            put(buf, x, y0, "─", line, bg, false);
            put(buf, x, y1, "─", line, bg, false);
        }
        for y in y0 + 1..y1 {
            put(buf, x0, y, "│", line, bg, false);
            put(buf, x1, y, "│", line, bg, false);
        }
        put(buf, x0, y0, "╭", line, bg, false);
        put(buf, x1, y0, "╮", line, bg, false);
        put(buf, x0, y1, "╰", line, bg, false);
        put(buf, x1, y1, "╯", line, bg, false);

        let (cols, rows) = (b.width - 2, b.height - 2);
        match &p.choices[p.sel] {
            Choice::Style(_) => {
                if let Some(px) = p.thumbs.get(&(p.sel, cols, rows)) {
                    for r in 0..rows {
                        for cx in 0..cols {
                            let top = px[(r * 2 * cols + cx) as usize];
                            let bottom = px[((r * 2 + 1) * cols + cx) as usize];
                            put(buf, x0 + 1 + cx, y0 + 1 + r, "▀", color(top), color(bottom), false);
                        }
                    }
                }
            }
            Choice::File(name) => {
                for r in 0..rows {
                    for cx in 0..cols {
                        put(buf, x0 + 1 + cx, y0 + 1 + r, " ", fg, color(c.surface), false);
                    }
                }
                let msg = "sua imagem";
                let mx = x0 + 1 + (cols.saturating_sub(msg.chars().count() as u16)) / 2;
                put(buf, mx, y0 + rows / 2, msg, dim, color(c.surface), false);
                let nx = x0 + 1 + (cols.saturating_sub(name.chars().count() as u16)) / 2;
                put(buf, nx, y0 + rows / 2 + 1, name, mute, color(c.surface), false);
            }
        }

        let (title, desc) = match &p.choices[p.sel] {
            Choice::Style(s) => (s.name().to_string(), s.desc().to_string()),
            Choice::File(n) => (n.clone(), "imagem sua, copiada para ~/.local/share/clios/wallpapers".to_string()),
        };
        put(buf, x0, b.bottom() + 1, &title, fg, bg, true);
        put(buf, x0, b.bottom() + 2, &desc, mute, bg, false);
    }

    // rodapé
    let hint = if p.notice.is_empty() {
        "↵ aplicar   ↑↓ escolher   r sortear   q sair".to_string()
    } else {
        p.notice.clone()
    };
    put(buf, PAD, geo.footer_y, &hint, if p.notice.is_empty() { mute } else { accent }, bg, false);
}

/// Um quadro do seletor como ANSI de 24 bits.
pub fn snapshot(ctx: &Ctx, size: &str, select: Option<&str>) -> Result<()> {
    let (w, h) = size
        .split_once('x')
        .and_then(|(w, h)| Some((w.parse::<u16>().ok()?, h.parse::<u16>().ok()?)))
        .ok_or_else(|| anyhow::anyhow!("tamanho inválido {size:?}, esperava LARGURAxALTURA (ex.: 100x30)"))?;
    let mut p = Picker::new(ctx, ctx.theme()?);
    if let Some(id) = select {
        p.sel = p.choices.iter().position(|c| c.id() == id).ok_or_else(|| anyhow::anyhow!("não há {id:?} na lista"))?;
    }
    let area = Rect::new(0, 0, w, h);
    p.prepare(&geometry(area));
    let mut buf = Buffer::empty(area);
    render(&mut buf, &p);
    print!("{}", crate::ui::buffer_ansi(&buf));
    Ok(())
}

pub fn run(ctx: &mut Ctx) -> Result<()> {
    let theme = ctx.theme()?;
    let mut picker = Picker::new(ctx, theme);
    let mut terminal = ratatui::init();
    let _ = execute!(io::stdout(), EnableMouseCapture);
    let result = event_loop(&mut terminal, ctx, &mut picker);
    let _ = execute!(io::stdout(), DisableMouseCapture);
    ratatui::restore();
    result
}

fn apply_selected(ctx: &mut Ctx, p: &mut Picker) {
    let choice = p.choices[p.sel].clone();
    ctx.state.wallpaper = choice.id();
    p.notice = match ctx.save_state().and_then(|()| wp::apply(ctx).map(|_| ())) {
        Ok(()) => {
            p.current = choice.id();
            format!("aplicado: {}", choice.label())
        }
        Err(e) => format!("não consegui aplicar: {e:#}"),
    };
}

fn event_loop(terminal: &mut ratatui::DefaultTerminal, ctx: &mut Ctx, p: &mut Picker) -> Result<()> {
    loop {
        let size = terminal.size()?;
        let geo = geometry(Rect::new(0, 0, size.width, size.height));
        p.prepare(&geo);
        terminal.draw(|f| render(f.buffer_mut(), p))?;
        if !event::poll(Duration::from_millis(500))? {
            continue;
        }
        match event::read()? {
            Event::Key(k) if k.kind != KeyEventKind::Release => match (k.code, k.modifiers) {
                (KeyCode::Char('q') | KeyCode::Esc, _) | (KeyCode::Char('c'), KeyModifiers::CONTROL) => return Ok(()),
                (KeyCode::Down | KeyCode::Char('j'), _) | (KeyCode::Tab, _) => p.move_by(1),
                (KeyCode::Up | KeyCode::Char('k'), _) | (KeyCode::BackTab, _) => p.move_by(-1),
                (KeyCode::Home | KeyCode::Char('g'), _) => p.sel = 0,
                (KeyCode::End | KeyCode::Char('G'), _) => p.sel = p.choices.len() - 1,
                (KeyCode::Enter | KeyCode::Char(' '), _) => apply_selected(ctx, p),
                (KeyCode::Char('r'), _) => {
                    let n = p.choices.len();
                    let t = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map_or(0, |d| d.as_millis() as usize);
                    p.sel = (p.sel + 1 + t % (n.max(2) - 1)) % n;
                    apply_selected(ctx, p);
                }
                _ => {}
            },
            Event::Mouse(m) => match m.kind {
                MouseEventKind::ScrollDown => p.move_by(1),
                MouseEventKind::ScrollUp => p.move_by(-1),
                MouseEventKind::Down(MouseButton::Left)
                    if m.column >= PAD
                        && m.column <= PAD + LIST_W
                        && m.row >= geo.list_top
                        && m.row < geo.list_top + geo.list_rows =>
                {
                    let first = first_visible(p.sel, geo.list_rows);
                    let idx = first + usize::from(m.row - geo.list_top);
                    if idx < p.choices.len() {
                        let again = idx == p.sel;
                        p.sel = idx;
                        if again {
                            apply_selected(ctx, p);
                        }
                    }
                }
                _ => {}
            },
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clios_core::wallpaper::Style;
    use std::path::Path;

    fn picker(name: &str) -> Picker {
        let home = std::env::temp_dir().join(format!("clios-wptui-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&home);
        std::fs::create_dir_all(&home).unwrap();
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let ctx = Ctx::load(Some(&root), Some(&home)).unwrap();
        let theme = ctx.theme().unwrap();
        Picker::new(&ctx, theme)
    }

    fn text(buf: &Buffer, y: u16) -> String {
        (0..buf.area.width).map(|x| buf[(x, y)].symbol().to_string()).collect()
    }

    #[test]
    fn renders_list_preview_and_description() {
        let mut p = picker("render");
        let area = Rect::new(0, 0, 100, 30);
        let geo = geometry(area);
        p.prepare(&geo);
        let mut buf = Buffer::empty(area);
        render(&mut buf, &p);
        let all: String = (0..30).map(|y| text(&buf, y)).collect::<Vec<_>>().join("\n");
        assert!(all.contains("papel de parede"));
        for s in Style::ALL {
            assert!(all.contains(s.name()), "falta {} na lista", s.name());
        }
        assert!(all.contains(Style::Grade.desc()), "a descrição da seleção aparece");
        assert!(all.contains('╭') && all.contains('╯'), "caixa arredondada");
        assert!(all.contains('●'), "marca o que está aplicado");
        // a miniatura são meio-blocos coloridos de verdade
        let b = geo.preview;
        let cell = &buf[(b.x + b.width / 2, b.y + b.height / 2)];
        assert_eq!(cell.symbol(), "▀");
        assert_ne!(cell.fg, cell.bg, "o meio da grade tem pontos, não pode ser chapado");
    }

    #[test]
    fn selection_wraps_and_selected_row_is_a_pill() {
        let mut p = picker("pill");
        p.sel = 0;
        p.move_by(-1);
        assert_eq!(p.sel, p.choices.len() - 1);
        p.move_by(1);
        assert_eq!(p.sel, 0);
        let area = Rect::new(0, 0, 100, 30);
        p.prepare(&geometry(area));
        let mut buf = Buffer::empty(area);
        render(&mut buf, &p);
        let row = text(&buf, geometry(area).list_top);
        assert!(row.contains(CAP_L) && row.contains(CAP_R) && row.contains('❯'), "{row:?}");
    }

    #[test]
    fn tiny_terminals_do_not_panic() {
        let mut p = picker("tiny");
        for (w, h) in [(10, 5), (40, 8), (100, 4), (0, 0)] {
            let area = Rect::new(0, 0, w, h);
            p.prepare(&geometry(area));
            let mut buf = Buffer::empty(area);
            render(&mut buf, &p);
        }
    }

    #[test]
    fn user_image_gets_a_placeholder_instead_of_a_thumbnail() {
        let mut p = picker("file");
        p.choices.push(Choice::File("praia.jpg".into()));
        p.sel = p.choices.len() - 1;
        let area = Rect::new(0, 0, 100, 30);
        p.prepare(&geometry(area));
        let mut buf = Buffer::empty(area);
        render(&mut buf, &p);
        let all: String = (0..30).map(|y| text(&buf, y)).collect::<Vec<_>>().join("\n");
        assert!(all.contains("sua imagem") && all.contains("praia.jpg"));
    }
}
