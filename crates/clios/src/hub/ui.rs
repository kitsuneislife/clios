//! Desenho do hub. Escreve direto no buffer; recebe o instante para as animações.
//!
//! Layout (x = coluna, pad = 3):
//!
//!       clios▌                                  ember · dark
//!
//!     ❯ busca▌
//!     ───────────────────────────────────────────────────────
//!    ❯ arquivos                                           tui      (a linha selecionada é uma pílula)
//!       editor                                            tui
//!       …
//!
//!     ↵ abrir   esc fechar   ⇥ escopo      @ janelas  ? atalhos

use std::time::{Duration, Instant};

use clios_core::Rgb;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use unicode_width::UnicodeWidthChar;

use super::anim::fade;
use super::app::App;
use super::items::{Item, Scope};

const PAD: u16 = 3;
/// Tampas redondas da pílula: glifos powerline da Nerd Font (o foot usa a GeistMono Nerd Font).
const CAP_L: &str = "\u{e0b6}";
const CAP_R: &str = "\u{e0b4}";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Layout {
    pub header_y: u16,
    pub prompt_y: u16,
    pub rule_y: u16,
    pub list_top: u16,
    pub list_rows: u16,
    pub footer_y: u16,
}

pub fn layout(area: Rect) -> Layout {
    let h = area.height;
    let footer_y = h.saturating_sub(2);
    let list_top = 5;
    Layout {
        header_y: 1,
        prompt_y: 3,
        rule_y: 4,
        list_top,
        // uma linha de respiro entre a lista e o rodapé
        list_rows: footer_y.saturating_sub(list_top + 1),
        footer_y,
    }
}

fn color(c: Rgb) -> Color {
    Color::Rgb(c.r, c.g, c.b)
}

fn style(fg: Rgb, bg: Rgb) -> Style {
    Style::default().fg(color(fg)).bg(color(bg))
}

fn text_width(s: &str) -> usize {
    s.chars().map(|c| c.width().unwrap_or(0)).sum()
}

/// Corta `s` para caber em `max` células, terminando em `…` se precisou cortar.
pub fn fit(s: &str, max: usize) -> String {
    if text_width(s) <= max {
        return s.to_string();
    }
    if max == 0 {
        return String::new();
    }
    let mut out = String::new();
    let mut w = 0;
    for c in s.chars() {
        let cw = c.width().unwrap_or(0);
        if w + cw + 1 > max {
            break;
        }
        out.push(c);
        w += cw;
    }
    out.push('…');
    out
}

fn put(buf: &mut Buffer, x: u16, y: u16, s: &str, st: Style) -> u16 {
    if y >= buf.area.bottom() || x >= buf.area.right() {
        return x;
    }
    buf.set_string(x, y, s, st);
    x.saturating_add(text_width(s) as u16)
}

fn fill_row(buf: &mut Buffer, y: u16, bg: Rgb) {
    let r = buf.area;
    for x in r.left()..r.right() {
        buf[(x, y)].set_bg(color(bg)).set_symbol(" ");
    }
}

pub fn render(buf: &mut Buffer, app: &App, now: Instant) {
    let area = buf.area;
    let t = &app.theme.c;
    let lay = layout(area);

    // fundo
    for y in area.top()..area.bottom() {
        fill_row(buf, y, t.bg);
        for x in area.left()..area.right() {
            buf[(x, y)].set_fg(color(t.fg));
        }
    }

    let x0 = PAD + 2; // coluna do texto (o marcador da seleção fica em PAD)
    let right = area.right().saturating_sub(PAD);

    // cabeçalho: nome + cursor de bloco (a marca) e o estado do tema
    let after = put(buf, x0, lay.header_y, "clios", style(t.fg, t.bg).add_modifier(Modifier::BOLD));
    put(buf, after, lay.header_y, "▌", style(t.accent, t.bg));
    let mut state = format!("{} · {}", app.theme.accent_name, app.theme.mode);
    if app.scope != Scope::All {
        state = format!("{} · {state}", app.scope.name());
    }
    let sx = right.saturating_sub(text_width(&state) as u16);
    put(buf, sx, lay.header_y, &state, style(t.mute, t.bg));

    // prompt
    put(buf, PAD, lay.prompt_y, "❯", style(t.accent, t.bg));
    let mut x = x0;
    let mut query = app.query.as_str();
    if app.scope != Scope::All {
        x = put(buf, x, lay.prompt_y, app.scope.prefix(), style(t.accent, t.bg).add_modifier(Modifier::BOLD));
        query = app.needle();
    }
    x = put(buf, x, lay.prompt_y, query, style(t.fg, t.bg));
    if app.cursor_visible(now) {
        put(buf, x, lay.prompt_y, "▌", style(t.accent, t.bg));
    }
    if app.query.is_empty() {
        put(buf, x + 2, lay.prompt_y, "buscar apps, ações, tudo", style(t.mute, t.bg));
    }

    // régua
    let rule = "─".repeat(usize::from(area.width.saturating_sub(PAD * 2)));
    put(buf, PAD, lay.rule_y, &rule, style(t.line, t.bg));

    render_rows(buf, app, now, &lay, x0, right);
    render_footer(buf, app, now, &lay, x0, right);
}

fn render_rows(buf: &mut Buffer, app: &App, now: Instant, lay: &Layout, x0: u16, right: u16) {
    let t = &app.theme.c;
    let m = &app.motion;

    if app.hits.is_empty() {
        let msg = match (app.scope, app.needle().is_empty()) {
            (Scope::Windows, true) => "nenhuma janela aberta",
            (Scope::Keys, true) => "nenhum atalho com descrição (o Hyprland está rodando?)",
            (Scope::Clipboard, true) => "histórico vazio (o cliphist está rodando?)",
            (Scope::Install, true) => "tudo do catálogo já está instalado",
            (Scope::Calc, true) => "uma conta: 2^10, 18% de 230, 5 km em milhas, 3 dias em horas",
            (Scope::Calc, false) => "não entendi a conta",
            (_, true) => "digite para buscar",
            (_, false) => "nada encontrado",
        };
        put(buf, x0, lay.list_top, msg, style(t.mute, t.bg));
        return;
    }

    let since_reveal = now.saturating_duration_since(app.reveal_from);
    let since_sel = now.saturating_duration_since(app.sel_changed);

    for row in 0..usize::from(lay.list_rows) {
        let pos = app.scroll + row;
        let Some(hit) = app.hits.get(pos) else { break };
        let item: &Item = &app.items[hit.index];
        let y = lay.list_top + row as u16;

        // entrada escalonada: cada linha começa um pouco depois da anterior
        let delay = Duration::from_millis(u64::from(m.stagger) * row.min(12) as u64);
        let reveal = if since_reveal < delay { 0.0 } else { m.progress(since_reveal - delay, m.fast) };

        // seleção: a antiga some enquanto a nova aparece
        let sel_a = if pos == app.sel {
            m.progress(since_sel, m.instant)
        } else if pos == app.sel_from {
            1.0 - m.progress(since_sel, m.instant)
        } else {
            0.0
        };

        // A seleção é uma pílula (tampas redondas) em vez de uma faixa de ponta a ponta.
        fill_row(buf, y, t.bg);
        let mut bg = t.bg;
        if sel_a > 0.0 {
            bg = fade(t.bg, t.raised, sel_a * reveal);
            for x in PAD..right {
                buf[(x, y)].set_bg(color(bg));
            }
            put(buf, PAD - 1, y, CAP_L, style(bg, t.bg));
            put(buf, right, y, CAP_R, style(bg, t.bg));
            put(buf, PAD, y, "❯", style(fade(bg, t.accent, sel_a * reveal), bg));
        }

        let base = fade(t.dim, t.fg, sel_a);
        let base = fade(bg, base, reveal);
        let hi = fade(bg, t.accent, reveal);
        let faint = fade(bg, t.mute, reveal);

        // lado direito primeiro, para saber quanto sobra ao título
        let armed = app.armed_for(&item.id, now);
        let mut right_text = String::new();
        let mut right_color = faint;
        if armed {
            right_text = "↵ confirmar".into();
            right_color = fade(bg, t.red, reveal);
        } else {
            if !item.hint.is_empty() {
                right_text.push_str(&item.hint);
            }
            if app.scope == Scope::All {
                if !right_text.is_empty() {
                    right_text.push_str("  ");
                }
                right_text.push_str(item.kind.label());
            }
        }
        let rw = text_width(&right_text) as u16;
        if rw > 0 {
            put(buf, right.saturating_sub(rw), y, &right_text, style(right_color, bg));
        }

        let mut x = x0;
        if let Some(sw) = item.swatch {
            put(buf, x, y, "●", style(fade(bg, sw, reveal), bg));
            x += 2;
        }
        let avail = usize::from(right.saturating_sub(x).saturating_sub(rw + 2));
        let title = fit(&item.title, avail);
        let bold = if pos == app.sel { Modifier::BOLD } else { Modifier::empty() };
        for (ci, ch) in title.chars().enumerate() {
            let lit = hit.title_idx.binary_search(&(ci as u32)).is_ok();
            let st = if lit { style(hi, bg).add_modifier(Modifier::BOLD) } else { style(base, bg).add_modifier(bold) };
            let mut tmp = [0u8; 4];
            x = put(buf, x, y, ch.encode_utf8(&mut tmp), st);
        }
    }
}

fn render_footer(buf: &mut Buffer, app: &App, now: Instant, lay: &Layout, x0: u16, right: u16) {
    let t = &app.theme.c;
    let y = lay.footer_y;

    let armed = app.selected_item().filter(|i| app.armed_for(&i.id, now));
    if let Some(item) = armed {
        let msg = format!("↵ de novo para confirmar: {}", item.title);
        put(buf, x0, y, &fit(&msg, usize::from(right.saturating_sub(x0))), style(t.red, t.bg));
        return;
    }
    if let Some(n) = &app.notice {
        put(buf, x0, y, &fit(n, usize::from(right.saturating_sub(x0))), style(t.yellow, t.bg));
        return;
    }

    let mut x = put(buf, x0, y, "↵", style(t.dim, t.bg));
    x = put(buf, x, y, if app.scope == Scope::Calc { " copiar   " } else { " abrir   " }, style(t.mute, t.bg));
    x = put(buf, x, y, "esc", style(t.dim, t.bg));
    x = put(buf, x, y, " fechar   ", style(t.mute, t.bg));
    x = put(buf, x, y, "⇥", style(t.dim, t.bg));
    put(buf, x, y, " escopo", style(t.mute, t.bg));

    let tail = if app.query.is_empty() {
        "@ janelas   ? atalhos   > ações   + apps".to_string()
    } else if app.hits.is_empty() {
        String::new()
    } else {
        format!("{}/{}", app.sel + 1, app.hits.len())
    };
    let w = text_width(&tail) as u16;
    // Não desenha por cima do que já está à esquerda em janelas estreitas.
    if right.saturating_sub(w) > x + 3 {
        put(buf, right - w, y, &tail, style(t.mute, t.bg));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hub::app::tests::{app, sample_items, theme};
    use crate::hub::items::{Action, Kind};
    use crate::hub::search::History;
    use clios_core::MotionLevel;

    fn draw(app: &App, now: Instant, w: u16, h: u16) -> Buffer {
        let mut buf = Buffer::empty(Rect::new(0, 0, w, h));
        render(&mut buf, app, now);
        buf
    }

    /// O texto da linha, sem as tampas da pílula (glifos que não são conteúdo).
    fn line(buf: &Buffer, y: u16) -> String {
        (0..buf.area.width)
            .map(|x| buf[(x, y)].symbol().to_string())
            .collect::<String>()
            .replace(['\u{e0b6}', '\u{e0b4}'], " ")
            .trim_end()
            .to_string()
    }

    /// Um instante em que toda animação já terminou.
    fn settled(app: &App) -> Instant {
        app.reveal_from.max(app.sel_changed).max(app.last_key) + Duration::from_secs(5)
    }

    #[test]
    fn layout_for_a_typical_window() {
        let l = layout(Rect::new(0, 0, 100, 30));
        assert_eq!((l.header_y, l.prompt_y, l.rule_y, l.list_top), (1, 3, 4, 5));
        assert_eq!(l.footer_y, 28);
        assert_eq!(l.list_rows, 22);
    }

    #[test]
    fn layout_never_underflows_in_a_tiny_window() {
        let l = layout(Rect::new(0, 0, 10, 3));
        assert_eq!(l.list_rows, 0);
        // E renderizar não pode entrar em pânico.
        let t0 = Instant::now();
        let a = app(t0);
        draw(&a, t0, 10, 3);
        draw(&a, t0, 1, 1);
        draw(&a, t0, 0, 0);
    }

    #[test]
    fn fit_truncates_with_ellipsis_by_cell_width() {
        assert_eq!(fit("firefox", 20), "firefox");
        assert_eq!(fit("firefox", 4), "fir…");
        assert_eq!(fit("firefox", 1), "…");
        assert_eq!(fit("firefox", 0), "");
        // caracteres largos ocupam duas células
        assert_eq!(fit("日本語日本語", 5), "日本…");
        assert_eq!(text_width(&fit("日本語日本語", 5)), 5);
    }

    #[test]
    fn home_screen_shows_brand_prompt_rows_and_hints() {
        let t0 = Instant::now();
        let a = app(t0);
        let b = draw(&a, settled(&a), 90, 26);
        assert!(line(&b, 1).contains("clios▌"), "{:?}", line(&b, 1));
        assert!(line(&b, 1).contains("ember · dark"));
        assert!(line(&b, 3).contains("❯"));
        assert!(line(&b, 3).contains("buscar apps, ações, tudo"));
        assert!(line(&b, 4).starts_with("   ─"));
        assert!(line(&b, 5).contains("arquivos") && line(&b, 5).ends_with("tui"));
        assert!(line(&b, 6).contains("editor"));
        assert!(line(&b, 7).contains("Firefox") && line(&b, 7).ends_with("app"));
        assert!(line(&b, 24).contains("↵ abrir") && line(&b, 24).contains("esc fechar"));
        assert!(line(&b, 24).contains("@ janelas") && line(&b, 24).contains("? atalhos"));
    }

    #[test]
    fn selected_row_is_a_rounded_pill_with_an_accent_marker() {
        let t0 = Instant::now();
        let a = app(t0);
        let b = draw(&a, settled(&a), 90, 26);
        let acc = &a.theme.c;
        // tampas redondas nas pontas, fundo `raised` entre elas, marcador no acento
        assert_eq!(b[(2, 5)].symbol(), CAP_L);
        assert_eq!(
            (b[(2, 5)].fg, b[(2, 5)].bg),
            (color(acc.raised), color(acc.bg)),
            "a tampa é a cor da pílula sobre o fundo"
        );
        assert_eq!(b[(3, 5)].symbol(), "❯");
        assert_eq!((b[(3, 5)].fg, b[(3, 5)].bg), (color(acc.accent), color(acc.raised)));
        assert_eq!(b[(40, 5)].bg, color(acc.raised));
        assert_eq!(b[(87, 5)].symbol(), CAP_R);
        assert_eq!(b[(86, 5)].bg, color(acc.raised));
        // e a pílula não vai de ponta a ponta: as margens ficam no fundo
        assert_eq!(b[(0, 5)].bg, color(acc.bg));
        assert_eq!(b[(89, 5)].bg, color(acc.bg));
        assert_eq!(b[(88, 5)].bg, color(acc.bg), "margem simétrica: 2 colunas de cada lado");
        // linha não selecionada: sem pílula
        assert_eq!(b[(3, 6)].symbol(), " ");
        assert_eq!(b[(40, 6)].bg, color(acc.bg));
    }

    #[test]
    fn typed_query_shows_highlight_and_count() {
        let t0 = Instant::now();
        let mut a = app(t0);
        a.set_query("fire", t0);
        let b = draw(&a, settled(&a), 90, 26);
        assert!(line(&b, 3).contains("fire"));
        let acc = color(a.theme.c.accent);
        // x0 = 5: "Fire" casou (acento), "fox" não (a linha está selecionada, então texto em fg)
        for x in 5..9 {
            assert_eq!(b[(x, 5)].fg, acc, "coluna {x}");
        }
        assert_eq!(b[(9, 5)].fg, color(a.theme.c.fg));
        assert!(line(&b, 24).contains("1/1"));
    }

    #[test]
    fn scope_prefix_is_drawn_in_accent_and_kind_label_hidden() {
        let t0 = Instant::now();
        let mut a = app(t0);
        a.add_items(vec![Item::new(Kind::Window, "w", "Docs", Action::None).hint("ws 2")], t0);
        a.set_query("@do", t0);
        let b = draw(&a, settled(&a), 90, 26);
        assert_eq!(b[(5, 3)].symbol(), "@");
        assert_eq!(b[(5, 3)].fg, color(a.theme.c.accent));
        assert!(line(&b, 1).contains("janelas · ember · dark"), "{:?}", line(&b, 1));
        let row = line(&b, 5);
        assert!(row.contains("Docs") && row.ends_with("ws 2"), "{row:?}");
        assert!(!row.contains("janela"), "o escopo já diz o que é");
    }

    #[test]
    fn empty_results_explain_themselves() {
        let t0 = Instant::now();
        let mut a = app(t0);
        a.set_query("zzzz", t0);
        assert!(line(&draw(&a, settled(&a), 90, 26), 5).contains("nada encontrado"));
        a.set_query("@", t0);
        assert!(line(&draw(&a, settled(&a), 90, 26), 5).contains("nenhuma janela aberta"));
    }

    #[test]
    fn armed_dangerous_action_turns_red_and_asks_again() {
        let t0 = Instant::now();
        let mut a = app(t0);
        a.set_query(">deslig", t0);
        let now = settled(&a);
        a.activate(0, now);
        let b = draw(&a, now, 90, 26);
        assert!(line(&b, 5).ends_with("↵ confirmar"), "{:?}", line(&b, 5));
        assert!(line(&b, 24).contains("↵ de novo para confirmar: desligar"));
        assert_eq!(b[(5, 24)].fg, color(a.theme.c.red));
        // depois da janela de 3s volta ao normal
        let b = draw(&a, now + Duration::from_secs(4), 90, 26);
        assert!(!line(&b, 24).contains("de novo"));
    }

    #[test]
    fn swatch_items_draw_a_colored_dot() {
        let t0 = Instant::now();
        let mut items = sample_items();
        items.push(
            Item::new(Kind::Action, "action:accent-azure", "acento: azure", Action::None)
                .swatch(Rgb::new(0x4C, 0x8D, 0xFF)),
        );
        let mut a = crate::hub::app::App::new(theme(MotionLevel::Full), items, History::default(), t0);
        a.live_sources = false;
        a.set_query(">acento", t0);
        let b = draw(&a, settled(&a), 90, 26);
        assert_eq!(b[(5, 5)].symbol(), "●");
        assert_eq!(b[(5, 5)].fg, Color::Rgb(0x4C, 0x8D, 0xFF));
    }

    #[test]
    fn rows_enter_staggered_then_settle() {
        let t0 = Instant::now();
        let a = app(t0);
        // t = 0: ninguém chegou ainda, o texto está invisível (cor == fundo)
        let b = draw(&a, t0, 90, 26);
        assert_eq!(b[(5, 5)].fg, color(a.theme.c.bg), "linha 0 começa no fundo");
        assert_eq!(b[(5, 7)].fg, color(a.theme.c.bg), "linha 2 também");
        // t = 100ms: linha 0 já avançou bem mais que a linha 2 (que só começou há 72ms)
        let b = draw(&a, t0 + Duration::from_millis(100), 90, 26);
        let lum = |c: Color| match c {
            Color::Rgb(r, g, bl) => Rgb::new(r, g, bl).luminance(),
            _ => panic!(),
        };
        assert!(lum(b[(5, 5)].fg) > lum(b[(5, 7)].fg), "a primeira linha lidera");
        // t = 2s: tudo assentado, texto na cor final
        let b = draw(&a, settled(&a), 90, 26);
        assert_eq!(b[(5, 6)].fg, color(a.theme.c.dim));
    }

    #[test]
    fn selection_crossfades_between_rows() {
        let t0 = Instant::now();
        let mut a = app(t0);
        let t1 = settled(&a);
        a.on_key(
            ratatui::crossterm::event::KeyEvent::new(
                ratatui::crossterm::event::KeyCode::Down,
                ratatui::crossterm::event::KeyModifiers::NONE,
            ),
            t1,
        );
        let mid = t1 + Duration::from_millis(45); // metade dos 90ms
        let b = draw(&a, mid, 90, 26);
        let (old_bg, new_bg) = (b[(40, 5)].bg, b[(40, 6)].bg);
        let bg = color(a.theme.c.bg);
        let raised = color(a.theme.c.raised);
        assert_ne!(old_bg, bg);
        assert_ne!(old_bg, raised, "a linha antiga está a meio caminho de sumir");
        assert_ne!(new_bg, bg);
        assert_ne!(new_bg, raised, "a nova está a meio caminho de aparecer");
        // e no fim, só a nova está realçada
        let b = draw(&a, t1 + Duration::from_millis(500), 90, 26);
        assert_eq!(b[(40, 5)].bg, bg);
        assert_eq!(b[(40, 6)].bg, raised);
    }

    #[test]
    fn motion_off_renders_the_final_frame_immediately() {
        let t0 = Instant::now();
        let mut a = crate::hub::app::App::new(theme(MotionLevel::Off), sample_items(), History::default(), t0);
        a.live_sources = false;
        a.refresh(t0);
        let b = draw(&a, t0, 90, 26);
        assert_eq!(b[(3, 5)].symbol(), "❯");
        assert_eq!(b[(5, 6)].fg, color(a.theme.c.dim), "sem esperar animação");
    }

    #[test]
    fn narrow_window_keeps_titles_inside_and_does_not_overlap_the_kind_label() {
        let t0 = Instant::now();
        let mut items = sample_items();
        items.push(Item::new(Kind::App, "app:long", "Um aplicativo com um nome absurdamente comprido", Action::None));
        let mut a = crate::hub::app::App::new(theme(MotionLevel::Full), items, History::default(), t0);
        a.live_sources = false;
        a.set_query("aplicativo", t0);
        let b = draw(&a, settled(&a), 40, 20);
        let row = line(&b, 5);
        assert!(row.contains('…'), "{row:?}");
        assert!(row.ends_with("app"), "{row:?}");
        assert!(row.chars().count() <= 40);
    }
}
