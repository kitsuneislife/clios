//! As seis páginas do guia. Tudo escreve direto no buffer: `render` recebe o instante para a entrada e o cursor.

use std::time::{Duration, Instant};

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

use super::draw::{
    PAD, Pen, code, code_width, fill, first_visible, is_shortcut, keycaps, keycaps_width, list_row, pill, put, reveal,
    rule, text_w, truncate, wrap,
};
use super::{App, Page, Setting};
use crate::art;
use crate::catalog::CATEGORIES;
use crate::sysinfo;

/// Largura da coluna de listas (grupos, categorias, configurações).
const LIST_W: u16 = 26;
const TOP: u16 = 4;

fn bottom(area: Rect) -> u16 {
    area.height.saturating_sub(3)
}

fn content_rows(area: Rect) -> u16 {
    bottom(area).saturating_sub(TOP)
}

/// Onde cada aba está: `(página, x, largura do texto)`.
pub fn tab_rects(_app: &App) -> Vec<(Page, u16, u16)> {
    let mut x = PAD + 11;
    Page::ALL
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let w = text_w(&format!("{} {}", i + 1, p.label())) + 2;
            let r = (*p, x, w);
            x += w + 2;
            r
        })
        .collect()
}

pub fn tab_at(app: &App, col: u16, row: u16) -> Option<Page> {
    if row != 1 {
        return None;
    }
    tab_rects(app).into_iter().find(|(_, x, w)| col + 1 >= *x && col <= x + w).map(|(p, ..)| p)
}

/// Seleciona o item principal da página (usado pelos cliques e pelo `--select` dos quadros de documentação).
pub fn set_sel(app: &mut App, i: usize) {
    match app.page {
        Page::Atalhos => app.group_sel = i.min(app.groups.len() - 1),
        Page::Apps => {
            app.cat_sel = i.min(CATEGORIES.len() - 1);
            app.app_sel = 0;
        }
        Page::Sistema => app.sys_sel = i.min(app.settings.len() - 1),
        Page::Dicas => app.tip = i.min(app.tips.len() - 1),
        _ => {}
    }
}

/// Trata um clique da página atual. `true` quando foi no que já estava selecionado (vira Enter).
pub fn click(app: &mut App, area: Rect, col: u16, row: u16) -> bool {
    let rows = content_rows(area);
    if row < TOP || row >= TOP + rows {
        return false;
    }
    let line = usize::from(row - TOP);
    let in_left = (PAD..PAD + LIST_W).contains(&col);
    let in_right = col >= PAD + LIST_W + 3;
    match app.page {
        Page::Atalhos if in_left => {
            let i = first_visible(app.group_sel, rows) + line;
            set_sel(app, i);
            false
        }
        Page::Apps if in_left => {
            let i = first_visible(app.cat_sel, rows) + line;
            if i < CATEGORIES.len() {
                set_sel(app, i);
            }
            false
        }
        Page::Apps if in_right => {
            let i = first_visible(app.app_sel, rows.saturating_sub(7)) + line;
            if i < app.apps_in_cat().len() {
                let again = i == app.app_sel;
                app.app_sel = i;
                return again;
            }
            false
        }
        Page::Sistema if in_left || in_right => {
            let hit = (0..app.settings.len()).find(|i| usize::from(sys_line(app, *i)) == line);
            if let Some(i) = hit {
                let again = i == app.sys_sel;
                app.sys_sel = i;
                return again;
            }
            false
        }
        _ => false,
    }
}

pub fn render(buf: &mut Buffer, app: &App, now: Instant) {
    let area = buf.area;
    let pen = Pen::of(&app.theme);
    fill(buf, area, pen.bg);
    header(buf, app, &pen);
    let body = Rect::new(PAD, TOP, area.width.saturating_sub(2 * PAD), content_rows(area));
    match app.page {
        Page::Inicio => inicio(buf, app, &pen, body, now),
        Page::Atalhos => atalhos(buf, app, &pen, body),
        Page::Apps => apps(buf, app, &pen, body),
        Page::Sistema => sistema(buf, app, &pen, body),
        Page::Dicas => dicas(buf, app, &pen, body),
        Page::Sobre => sobre(buf, app, &pen, body, now),
    }
    footer(buf, app, &pen, now);

    if app.theme.motion.enabled {
        let elapsed = now.duration_since(app.page_at).as_millis() as u64;
        let (stagger, fade) = (u64::from(app.motion.stagger), u64::from(app.motion.fast).max(1));
        let ease = |t: f64| app.motion.curve.ease(t);
        reveal(buf, body, pen.bg, elapsed, stagger, fade, &ease);
    }
}

fn header(buf: &mut Buffer, app: &App, pen: &Pen) {
    let area = buf.area;
    let x = put(buf, PAD, 1, "clios", pen.fg, pen.bg, true);
    put(buf, x, 1, "▌", pen.accent, pen.bg, false);
    for (i, (p, x, w)) in tab_rects(app).into_iter().enumerate() {
        let label = format!("{} {}", i + 1, p.label());
        if p == app.page {
            pill(buf, x, 1, w, pen.raised, pen.bg);
            put(buf, x + 1, 1, &label, pen.fg, pen.raised, true);
        } else {
            put(buf, x + 1, 1, &label, pen.mute, pen.bg, false);
        }
    }
    let right = format!("{} · {}", app.theme.accent_name, app.theme.mode);
    put(buf, area.width.saturating_sub(PAD + text_w(&right)), 1, &right, pen.mute, pen.bg, false);
    rule(buf, PAD, area.width.saturating_sub(PAD), 2, pen);
}

fn footer(buf: &mut Buffer, app: &App, pen: &Pen, now: Instant) {
    let area = buf.area;
    let y = area.height.saturating_sub(2);
    if let Some((msg, at)) = &app.notice {
        if now.duration_since(*at) < Duration::from_millis(3500) {
            put(buf, PAD, y, &truncate(msg, area.width.saturating_sub(2 * PAD)), pen.accent, pen.bg, false);
            return;
        }
    }
    let hint = match app.page {
        Page::Inicio => "↵ ver os atalhos   tab páginas   r atualizar",
        Page::Atalhos => "↑↓ grupo   tab páginas",
        Page::Apps => "↑↓ app   ←→ categoria   ↵ abrir ou instalar   r atualizar",
        Page::Sistema => "↑↓ escolher   ←→ mudar   ↵ aplicar",
        Page::Dicas => "←→ dica   espaço próxima",
        Page::Sobre => "tab páginas",
    };
    let hint = format!("{hint}   q sair");
    put(buf, PAD, y, &truncate(&hint, area.width.saturating_sub(2 * PAD)), pen.mute, pen.bg, false);
}

// ── início ────────────────────────────────────────────────────────────────

fn inicio(buf: &mut Buffer, app: &App, pen: &Pen, body: Rect, now: Instant) {
    let art_rows = if body.height >= 20 {
        10
    } else if body.height >= 12 {
        6
    } else {
        0
    };
    let right_x = if art_rows > 0 { body.x + art_rows as u16 * 2 + 5 } else { body.x };
    let right_w = body.right().saturating_sub(right_x);

    if art_rows > 0 {
        let cells = art::mark_cells(&app.theme, art_rows, app.cursor_on(now), pen.bg);
        let w = (art_rows * 2) as usize;
        for (i, c) in cells.iter().enumerate() {
            let (cx, cy) = (body.x + 1 + (i % w) as u16, body.y + 1 + (i / w) as u16);
            put(buf, cx, cy, "▀", c.top, c.bottom, false);
        }
        // um resumo discreto do sistema, embaixo da marca
        let mut y = body.y + art_rows as u16 + 3;
        let lines = [
            Some(app.info.os.clone()).filter(|s| !s.is_empty()),
            (!app.info.kernel.is_empty()).then(|| format!("kernel {}", app.info.kernel)),
            app.info.uptime.map(|u| format!("ligado há {}", sysinfo::fmt_duration(u))),
            app.info.packages.map(|n| format!("{n} pacotes")),
        ];
        for l in lines.into_iter().flatten() {
            if y < body.bottom() {
                put(buf, body.x + 1, y, &truncate(&l, art_rows as u16 * 2 + 2), pen.mute, pen.bg, false);
                y += 1;
            }
        }
    }

    let who = if app.info.user.is_empty() { String::new() } else { format!(", {}", app.info.user) };
    put(buf, right_x, body.y + 1, &format!("{}{who}", sysinfo::greeting(app.hour)), pen.fg, pen.bg, true);
    put(buf, right_x, body.y + 2, "Seu desktop é feito para o terminal. Comece por aqui.", pen.dim, pen.bg, false);

    let done = app.steps.iter().filter(|s| s.done == Some(true)).count();
    let counted = app.steps.iter().filter(|s| s.done.is_some()).count();
    put(buf, right_x, body.y + 4, "primeiros passos", pen.mute, pen.bg, false);
    let count = format!("{done} de {counted}");
    put(buf, right_x + right_w.saturating_sub(text_w(&count)), body.y + 4, &count, pen.mute, pen.bg, false);
    rule(buf, right_x, right_x + right_w, body.y + 5, pen);

    for (i, step) in app.steps.iter().enumerate() {
        let y = body.y + 6 + 2 * i as u16;
        if y >= body.bottom() {
            break;
        }
        let (icon, icon_fg) = match step.done {
            Some(true) => ("✓", pen.green),
            Some(false) => ("○", pen.mute),
            None => ("›", pen.accent),
        };
        put(buf, right_x, y, icon, icon_fg, pen.bg, step.done == Some(true));
        let title_fg = if step.done == Some(true) { pen.mute } else { pen.fg };
        put(buf, right_x + 2, y, &truncate(step.title, right_w.saturating_sub(4)), title_fg, pen.bg, false);
        // o atalho, à direita (só se couber)
        let kw = if is_shortcut(step.keys) { keycaps_width(step.keys) } else { code_width(step.keys) };
        if right_w > kw + text_w(step.title) + 6 && step.done != Some(true) {
            let kx = right_x + right_w - kw;
            if is_shortcut(step.keys) {
                keycaps(buf, kx, y, step.keys, pen, pen.bg);
            } else {
                code(buf, kx, y, step.keys, pen, pen.bg);
            }
        }
    }
}

// ── atalhos ───────────────────────────────────────────────────────────────

fn atalhos(buf: &mut Buffer, app: &App, pen: &Pen, body: Rect) {
    let rows = body.height;
    let first = first_visible(app.group_sel, rows);
    for (row, (i, g)) in app.groups.iter().enumerate().skip(first).take(usize::from(rows)).enumerate() {
        list_row(
            buf,
            pen,
            (body.x, body.y + row as u16, LIST_W),
            i == app.group_sel,
            &g.name,
            &g.key.len().to_string(),
        );
    }
    let x = body.x + LIST_W + 3;
    let w = body.right().saturating_sub(x);
    let g = &app.groups[app.group_sel];
    put(buf, x, body.y, &g.name, pen.fg, pen.bg, true);
    put(buf, x, body.y + 1, &truncate(&g.blurb, w), pen.mute, pen.bg, false);
    rule(buf, x, x + w, body.y + 2, pen);

    let keys_w = g.key.iter().map(|k| keycaps_width(&k.keys)).max().unwrap_or(0).min(w / 2);
    let max_rows = body.height.saturating_sub(4);
    for (i, k) in g.key.iter().enumerate().take(usize::from(max_rows)) {
        let y = body.y + 4 + i as u16;
        keycaps(buf, x, y, &k.keys, pen, pen.bg);
        put(buf, x + keys_w + 3, y, &truncate(&k.what, w.saturating_sub(keys_w + 3)), pen.dim, pen.bg, false);
    }
    if g.key.len() > usize::from(max_rows) {
        put(buf, x, body.bottom().saturating_sub(1), "… tem mais: super + /", pen.mute, pen.bg, false);
    }
}

// ── apps ──────────────────────────────────────────────────────────────────

fn apps(buf: &mut Buffer, app: &App, pen: &Pen, body: Rect) {
    let rows = body.height;

    // à esquerda: as categorias, com quantos apps de cada já estão instalados
    let first = first_visible(app.cat_sel, rows);
    for (row, i) in (first..CATEGORIES.len()).take(usize::from(rows)).enumerate() {
        let (id, name) = CATEGORIES[i];
        let in_cat: Vec<usize> =
            app.catalog.tui.iter().enumerate().filter(|(_, t)| t.category == id).map(|(k, _)| k).collect();
        let have = in_cat.iter().filter(|k| app.installed[**k]).count();
        let count = format!("{have}/{}", in_cat.len());
        list_row(buf, pen, (body.x, body.y + row as u16, LIST_W), i == app.cat_sel, name, &count);
    }

    // à direita: os apps da categoria e, embaixo, o detalhe do selecionado
    let x = body.x + LIST_W + 3;
    let w = body.right().saturating_sub(x);
    let in_cat = app.apps_in_cat();
    let list_rows = rows.saturating_sub(7);
    let first = first_visible(app.app_sel, list_rows);
    let name_w = 18.min(w / 3);
    for (row, (i, k)) in in_cat.iter().enumerate().skip(first).take(usize::from(list_rows)).enumerate() {
        let t = &app.catalog.tui[*k];
        let ok = app.installed[*k];
        let y = body.y + row as u16;
        let selected = i == app.app_sel;
        let row_bg = if selected { pen.raised } else { pen.bg };
        if selected {
            pill(buf, x, y, w.saturating_sub(1), pen.raised, pen.bg);
            put(buf, x, y, "❯", pen.accent, row_bg, true);
        }
        let (mark, mark_fg) = if ok { ("✓", pen.green) } else { ("·", pen.mute) };
        put(buf, x + 2, y, mark, mark_fg, row_bg, false);
        let name_fg = if !ok {
            pen.mute
        } else if selected {
            pen.fg
        } else {
            pen.dim
        };
        put(buf, x + 4, y, &truncate(&t.name, name_w), name_fg, row_bg, selected);
        let desc_x = x + 4 + name_w + 2;
        put(buf, desc_x, y, &truncate(&t.desc, (x + w).saturating_sub(desc_x + 2)), pen.mute, row_bg, false);
    }

    if let Some(k) = app.selected_app() {
        let t = &app.catalog.tui[k];
        let ok = app.installed[k];
        let y0 = body.bottom().saturating_sub(5);
        rule(buf, x, x + w, y0, pen);
        let mut y = y0 + 1;
        for line in wrap(&t.desc, w).into_iter().take(2) {
            put(buf, x, y, &line, pen.fg, pen.bg, false);
            y += 1;
        }
        if !t.tip.is_empty() && y < body.bottom() {
            put(buf, x, y, &truncate(&format!("dica: {}", t.tip), w), pen.mute, pen.bg, false);
            y += 1;
        }
        if y < body.bottom() {
            let (text, fg) = if ok {
                (format!("↵ abrir   (clios open {})", t.id), pen.accent)
            } else if t.pkg.is_empty() {
                ("sem pacote para instalar".to_string(), pen.mute)
            } else {
                (format!("↵ instalar   ({})", t.pkg), pen.yellow)
            };
            put(buf, x, y, &truncate(&text, w), fg, pen.bg, false);
        }
    }
}

// ── sistema ───────────────────────────────────────────────────────────────

/// A linha (relativa ao topo do corpo) de cada configuração: as ações da sessão ficam depois de uma linha em branco.
fn sys_line(app: &App, i: usize) -> u16 {
    let first_action = app.settings.iter().position(|s| s.is_action()).unwrap_or(usize::MAX);
    i as u16 + u16::from(i >= first_action)
}

fn sistema(buf: &mut Buffer, app: &App, pen: &Pen, body: Rect) {
    let w = LIST_W + 22;
    for (i, s) in app.settings.iter().enumerate() {
        let y = body.y + sys_line(app, i);
        if y >= body.bottom() {
            break;
        }
        let selected = i == app.sys_sel;
        let bg = if selected { pen.raised } else { pen.bg };
        if selected {
            pill(buf, body.x, y, w, pen.raised, pen.bg);
            put(buf, body.x, y, "❯", pen.accent, bg, true);
        }
        put(buf, body.x + 2, y, s.label(), if selected { pen.fg } else { pen.dim }, bg, selected);
        let value = app.setting_value(*s);
        let confirming = app.confirm == Some(*s);
        let shown = if confirming {
            "confirmar?".to_string()
        } else if selected && !s.is_action() && *s != Setting::Wallpaper {
            format!("‹ {value} ›")
        } else {
            value
        };
        let vx = body.x + w.saturating_sub(text_w(&shown) + 2);
        let fg = if confirming {
            pen.red
        } else if s.is_action() {
            pen.mute
        } else if selected {
            pen.accent
        } else {
            pen.fg
        };
        if *s == Setting::Accent {
            put(buf, vx.saturating_sub(3), y, "██", app.theme.c.accent, bg, false);
        }
        put(buf, vx, y, &shown, fg, bg, selected);
    }

    // à direita: a paleta de verdade, para ver o que muda
    let px = body.x + w + 6;
    if body.right() > px + 22 {
        put(buf, px, body.y, "paleta", pen.mute, pen.bg, false);
        rule(buf, px, body.right(), body.y + 1, pen);
        let c = &app.theme.c;
        let colors = [
            ("fundo", c.bg),
            ("recuado", c.surface),
            ("elevado", c.raised),
            ("linha", c.line),
            ("apagado", c.mute),
            ("suave", c.dim),
            ("texto", c.fg),
            ("acento", c.accent),
        ];
        for (i, (name, col)) in colors.iter().enumerate() {
            let y = body.y + 2 + i as u16;
            put(buf, px, y, "████", *col, pen.bg, false);
            put(buf, px + 5, y, name, pen.dim, pen.bg, false);
            put(buf, px + 14, y, &col.hex(), pen.mute, pen.bg, false);
        }
        // as cores de estado, lado a lado
        let y = body.y + 11;
        let mut x = px;
        for col in [c.red, c.green, c.yellow, c.blue, c.magenta, c.cyan] {
            x = put(buf, x, y, "███", col, pen.bg, false) + 1;
        }
        put(buf, px, y + 2, "As mudanças valem na hora, em todos os apps.", pen.mute, pen.bg, false);
    }
}

// ── dicas ─────────────────────────────────────────────────────────────────

fn dicas(buf: &mut Buffer, app: &App, pen: &Pen, body: Rect) {
    let t = &app.tips[app.tip];
    let card_w = body.width.min(68);
    let x = body.x + (body.width.saturating_sub(card_w)) / 2;
    let centered = |buf: &mut Buffer, y: u16, s: &str, fg, bold| {
        let cx = x + card_w.saturating_sub(text_w(s)) / 2;
        put(buf, cx, y, s, fg, pen.bg, bold);
    };
    let mut y = body.y + 2;
    centered(buf, y, &format!("dica {} de {}", app.tip + 1, app.tips.len()), pen.mute, false);
    y += 2;
    centered(buf, y, &t.title, pen.accent, true);
    y += 2;
    for line in wrap(&t.body, card_w) {
        centered(buf, y, &line, pen.fg, false);
        y += 1;
    }
    if let Some(keys) = &t.keys {
        y += 1;
        if is_shortcut(keys) {
            let kw = keycaps_width(keys);
            keycaps(buf, x + card_w.saturating_sub(kw) / 2, y, keys, pen, pen.bg);
        } else {
            let kw = code_width(keys);
            code(buf, x + card_w.saturating_sub(kw) / 2, y, keys, pen, pen.bg);
        }
        y += 1;
    }
    // os pontinhos: onde você está no baralho
    let dots_y = (y + 2).min(body.bottom().saturating_sub(1));
    let n = app.tips.len() as u16;
    let dx = x + card_w.saturating_sub(n * 2) / 2;
    for i in 0..n {
        let on = usize::from(i) == app.tip;
        put(buf, dx + i * 2, dots_y, if on { "●" } else { "·" }, if on { pen.accent } else { pen.line }, pen.bg, false);
    }
}

// ── sobre ─────────────────────────────────────────────────────────────────

const PRINCIPLES: [(&str, &str); 5] = [
    ("leveza", "pouca coisa rodando, e tudo rápido"),
    ("limpeza", "nada de enfeite que não serve a ninguém"),
    ("minimalismo", "preto no branco, branco no preto, e um acento seu"),
    ("coerência", "uma fonte de tokens pinta o desktop inteiro"),
    ("movimento", "curvas e tempos de uma só família, e dá para desligar"),
];

const STACK: [(&str, &str); 8] = [
    ("hyprland", "janelas"),
    ("quickshell", "barra e avisos"),
    ("foot", "terminal"),
    ("fish", "shell"),
    ("helix", "editor"),
    ("yazi", "arquivos"),
    ("lazygit", "git"),
    ("rust", "as nossas ferramentas"),
];

fn sobre(buf: &mut Buffer, app: &App, pen: &Pen, body: Rect, now: Instant) {
    let art_rows = if body.height >= 18 { 8 } else { 0 };
    let x = if art_rows > 0 { body.x + art_rows as u16 * 2 + 5 } else { body.x };
    let w = body.right().saturating_sub(x);
    if art_rows > 0 {
        let cells = art::mark_cells(&app.theme, art_rows, app.cursor_on(now), pen.bg);
        let cw = (art_rows * 2) as usize;
        for (i, c) in cells.iter().enumerate() {
            put(buf, body.x + 1 + (i % cw) as u16, body.y + 1 + (i / cw) as u16, "▀", c.top, c.bottom, false);
        }
    }
    let nx = put(buf, x, body.y + 1, "clios", pen.fg, pen.bg, true);
    put(buf, nx + 1, body.y + 1, concat!("v", env!("CARGO_PKG_VERSION")), pen.mute, pen.bg, false);
    put(buf, x, body.y + 2, "um desktop Linux feito para o terminal", pen.dim, pen.bg, false);

    put(buf, x, body.y + 4, "princípios", pen.mute, pen.bg, false);
    rule(buf, x, x + w, body.y + 5, pen);
    for (i, (name, what)) in PRINCIPLES.iter().enumerate() {
        let y = body.y + 6 + i as u16;
        put(buf, x, y, name, pen.accent, pen.bg, false);
        put(buf, x + 14, y, &truncate(what, w.saturating_sub(14)), pen.dim, pen.bg, false);
    }

    let y0 = body.y + 12;
    if y0 + 2 < body.bottom() {
        put(buf, x, y0, "a pilha", pen.mute, pen.bg, false);
        rule(buf, x, x + w, y0 + 1, pen);
        let mut cx = x;
        let mut cy = y0 + 2;
        for (name, what) in STACK {
            let item = format!("{name} {what}");
            if cx + text_w(&item) + 2 > x + w {
                cx = x;
                cy += 1;
            }
            if cy >= body.bottom() {
                break;
            }
            let nx = put(buf, cx, cy, name, pen.fg, pen.bg, false);
            cx = put(buf, nx + 1, cy, what, pen.mute, pen.bg, false) + 3;
        }
        if cy + 2 < body.bottom() {
            put(buf, x, cy + 2, "github.com/kitsuneislife/clios  ·  MIT", pen.mute, pen.bg, false);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ctx::Ctx;
    use std::path::Path;

    fn app(name: &str, page: Page) -> App {
        let home = std::env::temp_dir().join(format!("clios-welcome-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&home);
        std::fs::create_dir_all(&home).unwrap();
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let ctx = Ctx::load(Some(&root), Some(&home)).unwrap();
        App::new(ctx, page, Instant::now()).unwrap()
    }

    fn draw(app: &App, w: u16, h: u16) -> String {
        let mut buf = Buffer::empty(Rect::new(0, 0, w, h));
        render(&mut buf, app, app.page_at + Duration::from_secs(5));
        (0..h)
            .map(|y| (0..w).map(|x| buf[(x, y)].symbol().to_string()).collect::<String>())
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn every_page_renders_at_every_size_without_panicking() {
        for page in Page::ALL {
            let a = app("sizes", page);
            for (w, h) in [(104, 32), (84, 24), (60, 16), (30, 8), (10, 4), (3, 2), (0, 0)] {
                draw(&a, w, h);
            }
        }
    }

    #[test]
    fn header_shows_every_tab_and_the_theme() {
        let a = app("header", Page::Inicio);
        let t = draw(&a, 104, 32);
        for p in Page::ALL {
            assert!(t.contains(p.label()), "falta a aba {}", p.label());
        }
        assert!(t.contains("ember · dark"));
    }

    #[test]
    fn home_greets_and_lists_the_first_steps() {
        let a = app("home", Page::Inicio);
        let t = draw(&a, 104, 32);
        assert!(t.contains("primeiros passos") && t.contains("abra o hub"), "{t}");
        assert!(t.contains("boa noite") || t.contains("bom dia") || t.contains("boa tarde"));
        assert!(t.contains('▀'), "a marca em meio-bloco");
    }

    #[test]
    fn shortcuts_page_draws_keycaps_and_descriptions() {
        let a = app("keys", Page::Atalhos);
        let t = draw(&a, 104, 32);
        assert!(t.contains("comece por aqui") && t.contains("abre o hub"), "{t}");
        assert!(t.contains(super::super::draw::CAP_L), "as teclas são pílulas");
        let mut b = app("keys2", Page::Atalhos);
        b.move_sel(1);
        assert!(draw(&b, 104, 32).contains("Cada letra abre"), "o grupo seguinte é o de apps");
    }

    #[test]
    fn apps_page_lists_categories_and_details() {
        let a = app("apps", Page::Apps);
        let t = draw(&a, 104, 32);
        assert!(t.contains("arquivos") && t.contains("pacotes"), "{t}");
        assert!(t.contains("yazi") || t.contains("arquivos"), "{t}");
        assert!(t.contains("↵ instalar") || t.contains("↵ abrir"), "o detalhe diz o que o Enter faz\n{t}");
    }

    #[test]
    fn system_page_shows_live_values_and_the_palette() {
        let a = app("sys", Page::Sistema);
        let t = draw(&a, 104, 32);
        assert!(t.contains("modo") && t.contains("escuro") && t.contains("ember"), "{t}");
        assert!(t.contains("paleta") && t.contains("#000000"), "{t}");
        assert!(t.contains("bloquear a tela") && t.contains("desligar"));
    }

    #[test]
    fn changing_a_setting_updates_state_and_theme_live() {
        let mut a = app("live", Page::Sistema);
        let now = Instant::now();
        a.sys_sel = 0;
        a.adjust(1, now);
        assert_eq!(a.theme.mode, clios_core::Mode::Light);
        assert_eq!(a.ctx.state.mode, clios_core::Mode::Light);
        assert!(draw(&a, 104, 32).contains("claro"));
        a.sys_sel = 1;
        a.adjust(1, now);
        assert_ne!(a.theme.accent_name, "ember");
        a.sys_sel = 2;
        a.adjust(1, now);
        assert_eq!(a.theme.motion.level, clios_core::MotionLevel::Reduced);
        a.adjust(-1, now);
        assert_eq!(a.theme.motion.level, clios_core::MotionLevel::Full);
        a.sys_sel = 3;
        a.adjust(1, now);
        assert_ne!(a.ctx.state.wallpaper, "grade");
    }

    #[test]
    fn dangerous_actions_need_a_second_enter() {
        let mut a = app("confirm", Page::Sistema);
        let now = Instant::now();
        let i = a.settings.iter().position(|s| *s == Setting::Poweroff).unwrap();
        a.sys_sel = i;
        a.activate(now);
        assert_eq!(a.confirm, Some(Setting::Poweroff));
        assert!(draw(&a, 104, 32).contains("confirmar?"));
        a.move_sel(-1);
        assert_eq!(a.confirm, None, "mexer na seleção cancela");
        // no sandbox nada é executado, nem na segunda vez
        a.sys_sel = i;
        a.activate(now);
        a.activate(now);
        assert!(a.notice.as_ref().unwrap().0.contains("simulação"));
    }

    #[test]
    fn tips_page_cycles_through_the_deck() {
        let mut a = app("tips", Page::Dicas);
        a.tip = 0;
        let first = draw(&a, 104, 32);
        assert!(first.contains(&format!("dica 1 de {}", a.tips.len())), "{first}");
        a.move_side(1, Instant::now());
        assert!(draw(&a, 104, 32).contains("dica 2 de"));
        a.tip = 0;
        a.move_side(-1, Instant::now());
        assert_eq!(a.tip, a.tips.len() - 1, "dá a volta");
    }

    #[test]
    fn about_page_lists_principles_and_stack() {
        let t = draw(&app("about", Page::Sobre), 104, 32);
        for s in ["leveza", "coerência", "hyprland", "quickshell", "github.com/kitsuneislife/clios"] {
            assert!(t.contains(s), "falta {s}\n{t}");
        }
    }

    #[test]
    fn keyboard_navigation_switches_pages() {
        use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};
        let mut a = app("nav", Page::Inicio);
        let now = Instant::now();
        let key = |c: KeyCode| KeyEvent {
            code: c,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        };
        a.on_key(key(KeyCode::Tab), now);
        assert_eq!(a.page, Page::Atalhos);
        a.on_key(key(KeyCode::Char('4')), now);
        assert_eq!(a.page, Page::Sistema);
        a.on_key(key(KeyCode::BackTab), now);
        assert_eq!(a.page, Page::Apps);
        for _ in 0..4 {
            a.on_key(key(KeyCode::Char(']')), now);
        }
        assert_eq!(a.page, Page::Inicio, "dá a volta");
        a.on_key(key(KeyCode::Char('q')), now);
        assert!(a.quit);
    }

    #[test]
    fn tabs_are_clickable() {
        let mut a = app("click", Page::Inicio);
        let rects = tab_rects(&a);
        let (page, x, w) = rects[3];
        assert_eq!(tab_at(&a, x + w / 2, 1), Some(page));
        assert_eq!(tab_at(&a, x + w / 2, 5), None, "só a linha das abas");
        let area = Rect::new(0, 0, 104, 32);
        a.go(Page::Sistema, Instant::now());
        // clicar numa configuração a seleciona; clicar de novo no mesmo é o Enter
        assert!(!click(&mut a, area, PAD + 3, TOP + 2));
        assert_eq!(a.sys_sel, 2);
        assert!(click(&mut a, area, PAD + 3, TOP + 2));
    }

    #[test]
    fn page_names_parse_with_or_without_accent() {
        assert_eq!(Page::parse("início").unwrap(), Page::Inicio);
        assert_eq!(Page::parse("inicio").unwrap(), Page::Inicio);
        assert_eq!(Page::parse("SISTEMA").unwrap(), Page::Sistema);
        assert!(Page::parse("nada").is_err());
    }
}
