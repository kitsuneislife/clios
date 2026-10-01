//! Estado do hub e o que cada tecla faz. Sem terminal aqui: tudo recebe o instante atual
//! como argumento, então os testes controlam o tempo.

use std::time::{Duration, Instant};

use clios_core::Theme;
use ratatui::crossterm::event::{
    KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};

use super::anim::Motion;
use super::items::{Action, Item, Kind, Scope};
use super::search::{History, Hit, Ranker};
use super::sources;

/// Quanto tempo o primeiro Enter de uma ação perigosa continua valendo.
pub const CONFIRM_WINDOW: Duration = Duration::from_secs(3);
/// Período de piscada do cursor de bloco.
pub const BLINK: Duration = Duration::from_millis(530);

pub enum Flow {
    Continue,
    Quit,
    /// Executar este item (índice em `App::items`) e sair.
    Run(usize),
}

pub struct App {
    pub theme: Theme,
    pub motion: Motion,
    pub items: Vec<Item>,
    pub query: String,
    pub scope: Scope,
    pub hits: Vec<Hit>,
    pub sel: usize,
    pub scroll: usize,
    /// Linhas visíveis na lista; a UI atualiza antes de cada tecla.
    pub view_rows: usize,
    pub history: History,
    pub armed: Option<(String, Instant)>,
    pub notice: Option<String>,

    pub reveal_from: Instant,
    pub sel_from: usize,
    pub sel_changed: Instant,
    pub last_key: Instant,

    ranker: Ranker,
    /// Segundos Unix atuais, para o histórico (injetável nos testes).
    pub clock: fn() -> u64,
    loaded: Vec<Scope>,
    /// Desliga as fontes do sistema (hyprctl, cliphist) nos testes.
    pub live_sources: bool,
}

fn unix_now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

impl App {
    pub fn new(theme: Theme, items: Vec<Item>, history: History, now: Instant) -> Self {
        let motion = Motion::from_theme(&theme);
        let mut app = Self {
            theme,
            motion,
            items,
            query: String::new(),
            scope: Scope::All,
            hits: Vec::new(),
            sel: 0,
            scroll: 0,
            view_rows: 12,
            history,
            armed: None,
            notice: None,
            reveal_from: now,
            sel_from: 0,
            sel_changed: now,
            last_key: now,
            ranker: Ranker::new(),
            clock: unix_now,
            loaded: Vec::new(),
            live_sources: true,
        };
        app.refresh(now);
        app
    }

    /// A consulta sem o prefixo de escopo.
    pub fn needle(&self) -> &str {
        Scope::split(&self.query).1
    }

    pub fn selected_item(&self) -> Option<&Item> {
        self.hits.get(self.sel).map(|h| &self.items[h.index])
    }

    /// Recalcula o escopo e os resultados. Carrega as fontes dinâmicas na primeira vez que o escopo aparece.
    pub fn refresh(&mut self, now: Instant) {
        let (scope, needle) = Scope::split(&self.query);
        let needle = needle.to_string();
        if scope != self.scope {
            self.scope = scope;
            self.reveal_from = now; // a lista nova entra com a mesma coreografia
        }
        self.ensure_loaded(scope);
        self.hits = self.ranker.rank(&self.items, scope, &needle, &self.history, (self.clock)());
        self.sel = self.sel.min(self.hits.len().saturating_sub(1));
        self.sel_from = self.sel;
        self.adjust_scroll();
    }

    fn ensure_loaded(&mut self, scope: Scope) {
        if !self.live_sources || self.loaded.contains(&scope) {
            return;
        }
        let (kind, fresh) = match scope {
            Scope::Windows => (Kind::Window, sources::windows()),
            Scope::Keys => (Kind::Key, sources::keys()),
            Scope::Clipboard => (Kind::Clip, sources::clipboard()),
            Scope::All | Scope::Actions => return,
        };
        self.items.retain(|i| i.kind != kind);
        self.items.extend(fresh);
        self.loaded.push(scope);
    }

    /// Para testes: injeta itens dinâmicos sem consultar o sistema.
    #[cfg(test)]
    pub fn add_items(&mut self, extra: Vec<Item>, now: Instant) {
        self.items.extend(extra);
        self.refresh(now);
    }

    /// A UI informa quantas linhas cabem; a rolagem se ajusta (ex.: janela redimensionada).
    pub fn set_view_rows(&mut self, rows: usize) {
        if rows != self.view_rows {
            self.view_rows = rows;
            self.adjust_scroll();
        }
    }

    fn adjust_scroll(&mut self) {
        let rows = self.view_rows.max(1);
        if self.sel < self.scroll {
            self.scroll = self.sel;
        } else if self.sel >= self.scroll + rows {
            self.scroll = self.sel + 1 - rows;
        }
        self.scroll = self.scroll.min(self.hits.len().saturating_sub(rows));
    }

    fn select(&mut self, to: usize, now: Instant) {
        let to = to.min(self.hits.len().saturating_sub(1));
        if to != self.sel {
            self.sel_from = self.sel;
            self.sel = to;
            self.sel_changed = now;
            self.adjust_scroll();
        }
        self.armed = None;
    }

    fn move_sel(&mut self, delta: isize, now: Instant) {
        if self.hits.is_empty() {
            return;
        }
        let n = self.hits.len() as isize;
        // Dá a volta: descer do último vai para o primeiro.
        let to = (self.sel as isize + delta).rem_euclid(n) as usize;
        self.select(to, now);
    }

    fn edit(&mut self, now: Instant, f: impl FnOnce(&mut String)) {
        f(&mut self.query);
        self.armed = None;
        self.notice = None;
        self.sel = 0;
        self.sel_from = 0;
        self.scroll = 0;
        self.refresh(now);
    }

    pub fn set_query(&mut self, q: &str, now: Instant) {
        self.edit(now, |s| *s = q.to_string());
    }

    pub fn on_key(&mut self, key: KeyEvent, now: Instant) -> Flow {
        if key.kind == KeyEventKind::Release {
            return Flow::Continue;
        }
        self.last_key = now;
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        let alt = key.modifiers.contains(KeyModifiers::ALT);
        let shift = key.modifiers.contains(KeyModifiers::SHIFT);

        match key.code {
            KeyCode::Esc => Flow::Quit,
            KeyCode::Char('c' | 'g' | 'q') if ctrl => Flow::Quit,
            KeyCode::Enter => self.activate(self.sel, now),
            KeyCode::Down => {
                self.move_sel(1, now);
                Flow::Continue
            }
            KeyCode::Char('n' | 'j') if ctrl => {
                self.move_sel(1, now);
                Flow::Continue
            }
            KeyCode::Up => {
                self.move_sel(-1, now);
                Flow::Continue
            }
            KeyCode::Char('p' | 'k') if ctrl => {
                self.move_sel(-1, now);
                Flow::Continue
            }
            KeyCode::PageDown => {
                self.select(self.sel + self.view_rows.max(1), now);
                Flow::Continue
            }
            KeyCode::PageUp => {
                self.select(self.sel.saturating_sub(self.view_rows.max(1)), now);
                Flow::Continue
            }
            KeyCode::Home => {
                self.select(0, now);
                Flow::Continue
            }
            KeyCode::End => {
                self.select(self.hits.len().saturating_sub(1), now);
                Flow::Continue
            }
            KeyCode::Tab | KeyCode::BackTab => {
                let back = key.code == KeyCode::BackTab || shift;
                let next = self.scope.next(back);
                let rest = self.needle().to_string();
                self.set_query(&format!("{}{}", next.prefix(), rest), now);
                Flow::Continue
            }
            KeyCode::Backspace if alt || ctrl => {
                self.edit(now, delete_word);
                Flow::Continue
            }
            KeyCode::Char('w') if ctrl => {
                self.edit(now, delete_word);
                Flow::Continue
            }
            KeyCode::Char('u') if ctrl => {
                self.edit(now, String::clear);
                Flow::Continue
            }
            KeyCode::Backspace => {
                self.edit(now, |s| {
                    s.pop();
                });
                Flow::Continue
            }
            KeyCode::Char(c) if !ctrl && !alt => {
                self.edit(now, |s| s.push(c));
                Flow::Continue
            }
            _ => Flow::Continue,
        }
    }

    /// Enter ou clique numa linha.
    pub fn activate(&mut self, pos: usize, now: Instant) -> Flow {
        let Some(hit) = self.hits.get(pos) else { return Flow::Continue };
        let idx = hit.index;
        let item = &self.items[idx];

        if let Action::Scope(s) = &item.action {
            let prefix = s.prefix().to_string();
            self.set_query(&prefix, now);
            return Flow::Continue;
        }
        if item.action == Action::None {
            return Flow::Continue;
        }
        if item.confirm {
            let valid =
                self.armed.as_ref().is_some_and(|(id, at)| *id == item.id && now.duration_since(*at) < CONFIRM_WINDOW);
            if !valid {
                self.armed = Some((item.id.clone(), now));
                return Flow::Continue;
            }
        }
        Flow::Run(idx)
    }

    pub fn on_mouse(&mut self, m: MouseEvent, list_top: usize, now: Instant) -> Flow {
        let row_hit = |app: &Self| -> Option<usize> {
            let row = usize::from(m.row).checked_sub(list_top)?;
            (row < app.view_rows).then_some(app.scroll + row).filter(|p| *p < app.hits.len())
        };
        match m.kind {
            MouseEventKind::ScrollDown => {
                self.move_sel(1, now);
                Flow::Continue
            }
            MouseEventKind::ScrollUp => {
                self.move_sel(-1, now);
                Flow::Continue
            }
            MouseEventKind::Moved => {
                if let Some(p) = row_hit(self) {
                    self.select(p, now);
                }
                Flow::Continue
            }
            MouseEventKind::Down(MouseButton::Left) => match row_hit(self) {
                Some(p) => {
                    self.select(p, now);
                    self.activate(p, now)
                }
                None => Flow::Continue,
            },
            _ => Flow::Continue,
        }
    }

    pub fn record_use(&mut self, idx: usize) {
        let id = self.items[idx].id.clone();
        self.history.record(&id, (self.clock)());
    }

    // ---- tempo -------------------------------------------------------------

    pub fn armed_for(&self, id: &str, now: Instant) -> bool {
        self.armed.as_ref().is_some_and(|(a, at)| a == id && now.duration_since(*at) < CONFIRM_WINDOW)
    }

    pub fn cursor_visible(&self, now: Instant) -> bool {
        let idle = now.saturating_duration_since(self.last_key);
        // Sólido enquanto você digita; só pisca quando para.
        idle < BLINK || (idle.as_millis() / BLINK.as_millis()) % 2 == 0
    }

    /// Há algo se movendo que exige quadros rápidos?
    pub fn animating(&self, now: Instant) -> bool {
        let visible = self.hits.len().min(self.view_rows.max(1));
        let reveal = now.saturating_duration_since(self.reveal_from) < self.motion.reveal_total(visible);
        let sel =
            now.saturating_duration_since(self.sel_changed) < Duration::from_millis(u64::from(self.motion.instant));
        reveal || sel
    }

    /// Quanto esperar até a próxima coisa que muda sozinha (piscada ou fim da confirmação).
    pub fn next_wake(&self, now: Instant) -> Duration {
        let idle = now.saturating_duration_since(self.last_key);
        let into_blink = Duration::from_millis((idle.as_millis() % BLINK.as_millis()) as u64);
        let mut wait = if idle < BLINK { BLINK - idle } else { BLINK - into_blink };
        if let Some((_, at)) = &self.armed {
            let left = CONFIRM_WINDOW.saturating_sub(now.duration_since(*at));
            wait = wait.min(left.max(Duration::from_millis(1)));
        }
        wait.max(Duration::from_millis(1))
    }
}

fn delete_word(s: &mut String) {
    let trimmed = s.trim_end().len();
    s.truncate(trimmed);
    match s.rfind(char::is_whitespace) {
        Some(i) => s.truncate(i + 1),
        None => s.clear(),
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use clios_core::{Mode, MotionLevel, Tokens};
    use ratatui::crossterm::event::KeyEventState;

    pub fn theme(level: MotionLevel) -> Theme {
        Theme::resolve(&Tokens::builtin(), Mode::Dark, "ember", level).unwrap()
    }

    fn it(kind: Kind, id: &str, title: &str, action: Action) -> Item {
        Item::new(kind, id, title, action)
    }

    pub fn sample_items() -> Vec<Item> {
        vec![
            it(Kind::Tui, "tui:files", "arquivos", Action::Gui("true".into())),
            it(Kind::Tui, "tui:editor", "editor", Action::Gui("true".into())),
            it(Kind::App, "app:firefox", "Firefox", Action::Gui("firefox".into())),
            it(Kind::Action, "action:lock", "bloquear", Action::Shell("hyprlock".into())),
            it(Kind::Action, "action:poweroff", "desligar", Action::Shell("systemctl poweroff".into())).confirm(),
            it(Kind::Action, "action:keys", "ver atalhos", Action::Scope(Scope::Keys)),
        ]
    }

    pub fn app(now: Instant) -> App {
        let mut a = App::new(theme(MotionLevel::Full), sample_items(), History::default(), now);
        a.live_sources = false;
        a.clock = || 1_000_000;
        a.refresh(now);
        a
    }

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent { code, modifiers: KeyModifiers::NONE, kind: KeyEventKind::Press, state: KeyEventState::NONE }
    }

    fn ctrl(c: char) -> KeyEvent {
        KeyEvent { modifiers: KeyModifiers::CONTROL, ..key(KeyCode::Char(c)) }
    }

    fn type_str(a: &mut App, s: &str, now: Instant) {
        for c in s.chars() {
            a.on_key(key(KeyCode::Char(c)), now);
        }
    }

    #[test]
    fn typing_filters_and_resets_selection() {
        let t = Instant::now();
        let mut a = app(t);
        a.on_key(key(KeyCode::Down), t);
        assert_eq!(a.sel, 1);
        type_str(&mut a, "fire", t);
        assert_eq!(a.query, "fire");
        assert_eq!(a.sel, 0);
        assert_eq!(a.selected_item().unwrap().title, "Firefox");
    }

    #[test]
    fn empty_query_starts_with_tuis_and_apps_only() {
        let a = app(Instant::now());
        let titles: Vec<_> = a.hits.iter().map(|h| a.items[h.index].title.as_str()).collect();
        assert_eq!(titles, ["arquivos", "editor", "Firefox"]);
    }

    #[test]
    fn selection_wraps_around() {
        let t = Instant::now();
        let mut a = app(t);
        a.on_key(key(KeyCode::Up), t);
        assert_eq!(a.sel, 2, "subir do primeiro vai para o último");
        a.on_key(key(KeyCode::Down), t);
        assert_eq!(a.sel, 0);
    }

    #[test]
    fn emacs_style_navigation() {
        let t = Instant::now();
        let mut a = app(t);
        a.on_key(ctrl('n'), t);
        a.on_key(ctrl('j'), t);
        assert_eq!(a.sel, 2);
        a.on_key(ctrl('p'), t);
        a.on_key(ctrl('k'), t);
        assert_eq!(a.sel, 0);
    }

    #[test]
    fn ctrl_chars_are_not_typed_into_the_query() {
        let t = Instant::now();
        let mut a = app(t);
        a.on_key(ctrl('n'), t);
        a.on_key(ctrl('x'), t);
        assert_eq!(a.query, "");
    }

    #[test]
    fn escape_and_ctrl_c_quit() {
        let t = Instant::now();
        let mut a = app(t);
        assert!(matches!(a.on_key(key(KeyCode::Esc), t), Flow::Quit));
        assert!(matches!(a.on_key(ctrl('c'), t), Flow::Quit));
    }

    #[test]
    fn backspace_ctrl_u_ctrl_w() {
        let t = Instant::now();
        let mut a = app(t);
        type_str(&mut a, "ver atalhos", t);
        a.on_key(key(KeyCode::Backspace), t);
        assert_eq!(a.query, "ver atalho");
        a.on_key(ctrl('w'), t);
        assert_eq!(a.query, "ver ");
        a.on_key(ctrl('u'), t);
        assert_eq!(a.query, "");
    }

    #[test]
    fn enter_runs_the_selected_item() {
        let t = Instant::now();
        let mut a = app(t);
        type_str(&mut a, "fire", t);
        match a.on_key(key(KeyCode::Enter), t) {
            Flow::Run(i) => assert_eq!(a.items[i].id, "app:firefox"),
            _ => panic!("esperava Run"),
        }
    }

    #[test]
    fn dangerous_action_needs_two_enters_within_the_window() {
        let t = Instant::now();
        let mut a = app(t);
        type_str(&mut a, ">desligar", t);
        assert!(matches!(a.on_key(key(KeyCode::Enter), t), Flow::Continue), "primeiro Enter só arma");
        assert!(a.armed_for("action:poweroff", t));
        assert!(matches!(a.on_key(key(KeyCode::Enter), t + Duration::from_secs(1)), Flow::Run(_)));
    }

    #[test]
    fn confirmation_expires() {
        let t = Instant::now();
        let mut a = app(t);
        type_str(&mut a, ">desligar", t);
        a.on_key(key(KeyCode::Enter), t);
        let late = t + CONFIRM_WINDOW + Duration::from_millis(1);
        assert!(!a.armed_for("action:poweroff", late));
        assert!(matches!(a.on_key(key(KeyCode::Enter), late), Flow::Continue), "expirou: arma de novo");
    }

    #[test]
    fn moving_or_typing_disarms() {
        let t = Instant::now();
        let mut a = app(t);
        type_str(&mut a, ">", t);
        a.set_query(">desligar", t);
        a.on_key(key(KeyCode::Enter), t);
        assert!(a.armed.is_some());
        a.on_key(key(KeyCode::Down), t);
        assert!(a.armed.is_none());
    }

    #[test]
    fn scope_item_switches_scope_without_quitting() {
        let t = Instant::now();
        let mut a = app(t);
        type_str(&mut a, "ver atalhos", t);
        assert!(matches!(a.on_key(key(KeyCode::Enter), t), Flow::Continue));
        assert_eq!(a.scope, Scope::Keys);
        assert_eq!(a.query, "?");
    }

    #[test]
    fn tab_cycles_scope_keeping_the_search_text() {
        let t = Instant::now();
        let mut a = app(t);
        type_str(&mut a, "foo", t);
        a.on_key(key(KeyCode::Tab), t);
        assert_eq!(a.query, "@foo");
        assert_eq!(a.scope, Scope::Windows);
        a.on_key(key(KeyCode::BackTab), t);
        assert_eq!(a.query, "foo");
    }

    #[test]
    fn informational_items_do_nothing_on_enter() {
        let t = Instant::now();
        let mut a = app(t);
        a.add_items(vec![it(Kind::Key, "key:0", "fechar janela", Action::None).hint("super + q")], t);
        a.set_query("?fechar", t);
        assert!(matches!(a.on_key(key(KeyCode::Enter), t), Flow::Continue));
    }

    #[test]
    fn scroll_follows_selection() {
        let t = Instant::now();
        let mut items = Vec::new();
        for i in 0..40 {
            items.push(it(Kind::App, &format!("app:{i}"), &format!("app {i:02}"), Action::Gui("true".into())));
        }
        let mut a = App::new(theme(MotionLevel::Full), items, History::default(), t);
        a.live_sources = false;
        a.view_rows = 10;
        a.refresh(t);
        a.on_key(key(KeyCode::PageDown), t);
        assert_eq!(a.sel, 10);
        assert!(a.scroll > 0 && a.sel < a.scroll + 10, "{} {}", a.scroll, a.sel);
        a.on_key(key(KeyCode::End), t);
        assert_eq!(a.sel, 39);
        assert_eq!(a.scroll, 30);
        a.on_key(key(KeyCode::Home), t);
        assert_eq!((a.sel, a.scroll), (0, 0));
    }

    #[test]
    fn mouse_click_activates_the_row_under_the_pointer() {
        let t = Instant::now();
        let mut a = app(t);
        let click = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 5,
            row: 7, // lista começa na linha 5 → terceira linha (índice 2)
            modifiers: KeyModifiers::NONE,
        };
        match a.on_mouse(click, 5, t) {
            Flow::Run(i) => assert_eq!(a.items[i].id, "app:firefox"),
            _ => panic!("esperava Run"),
        }
    }

    #[test]
    fn mouse_outside_the_list_is_ignored_and_hover_selects() {
        let t = Instant::now();
        let mut a = app(t);
        let ev = |kind, row| MouseEvent { kind, column: 3, row, modifiers: KeyModifiers::NONE };
        assert!(matches!(a.on_mouse(ev(MouseEventKind::Down(MouseButton::Left), 1), 5, t), Flow::Continue));
        a.on_mouse(ev(MouseEventKind::Moved, 6), 5, t);
        assert_eq!(a.sel, 1);
        a.on_mouse(ev(MouseEventKind::ScrollDown, 0), 5, t);
        assert_eq!(a.sel, 2);
    }

    #[test]
    fn cursor_is_solid_while_typing_and_blinks_when_idle() {
        let t = Instant::now();
        let mut a = app(t);
        a.on_key(key(KeyCode::Char('a')), t);
        assert!(a.cursor_visible(t + Duration::from_millis(200)));
        assert!(!a.cursor_visible(t + BLINK + Duration::from_millis(10)), "fase apagada");
        assert!(a.cursor_visible(t + BLINK * 2 + Duration::from_millis(10)), "fase acesa");
    }

    #[test]
    fn animation_runs_on_open_then_settles() {
        let t = Instant::now();
        let a = app(t);
        assert!(a.animating(t + Duration::from_millis(50)));
        assert!(!a.animating(t + Duration::from_secs(2)));
    }

    #[test]
    fn motion_off_means_nothing_animates() {
        let t = Instant::now();
        let mut a = App::new(theme(MotionLevel::Off), sample_items(), History::default(), t);
        a.live_sources = false;
        a.refresh(t);
        assert!(!a.animating(t));
    }

    #[test]
    fn next_wake_is_never_zero_and_never_exceeds_a_blink() {
        let t = Instant::now();
        let a = app(t);
        for ms in [0u64, 100, 529, 530, 531, 1000, 5000] {
            let w = a.next_wake(t + Duration::from_millis(ms));
            assert!(w >= Duration::from_millis(1) && w <= BLINK, "{ms}: {w:?}");
        }
    }

    #[test]
    fn using_an_item_records_history() {
        let t = Instant::now();
        let mut a = app(t);
        let idx = a.items.iter().position(|i| i.id == "action:lock").unwrap();
        a.record_use(idx);
        assert!(a.history.used("action:lock"));
        // E agora ela aparece na tela inicial.
        a.refresh(t);
        assert!(a.hits.iter().any(|h| a.items[h.index].id == "action:lock"));
    }
}
