//! O guia de boas-vindas e a central do sistema: o primeiro lugar onde o CLIOS se explica.
//!
//! Seis páginas num mesmo terminal: início (com os primeiros passos), atalhos, apps, sistema
//! (as configurações de verdade, ao vivo), dicas e sobre. Abre sozinho no primeiro login
//! (`clios welcome --first-run`) e depois é só `clios welcome`, ou `SUPER + F10`.

pub mod data;
pub mod draw;
pub mod pages;

use std::io;
use std::time::{Duration, Instant};

use anyhow::{Result, bail};
use clios_core::GreetMode;
use clios_core::{Mode, MotionLevel, Theme};
use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{
    self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind, KeyModifiers, MouseButton,
    MouseEventKind,
};
use ratatui::crossterm::execute;
use ratatui::layout::Rect;

use crate::catalog::{CATEGORIES, Catalog};
use crate::cmd::toggles::{self, Switch, state_word};
use crate::cmd::{theme as theme_cmd, wallpaper as wp};
use crate::ctx::Ctx;
use crate::hub::anim::Motion;
use crate::hub::exec;
use crate::hub::items::Action;
use crate::sysinfo::{self, Info};
use data::{Group, Probes, Step, Tip};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Page {
    Inicio,
    Atalhos,
    Apps,
    Sistema,
    Dicas,
    Sobre,
}

impl Page {
    pub const ALL: [Page; 6] = [Page::Inicio, Page::Atalhos, Page::Apps, Page::Sistema, Page::Dicas, Page::Sobre];

    pub fn label(self) -> &'static str {
        match self {
            Page::Inicio => "início",
            Page::Atalhos => "atalhos",
            Page::Apps => "apps",
            Page::Sistema => "sistema",
            Page::Dicas => "dicas",
            Page::Sobre => "sobre",
        }
    }

    pub fn parse(s: &str) -> Result<Page> {
        let s = s.to_lowercase();
        let norm = s.replace(['í', 'i'], "i");
        Page::ALL.into_iter().find(|p| p.label().replace('í', "i") == norm).ok_or_else(|| {
            anyhow::anyhow!("página {s:?} não existe. Páginas: início, atalhos, apps, sistema, dicas, sobre")
        })
    }

    fn index(self) -> usize {
        Page::ALL.iter().position(|p| *p == self).unwrap_or(0)
    }
}

/// O que a central do sistema deixa mexer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Setting {
    Mode,
    Accent,
    Motion,
    Wallpaper,
    Prompt,
    Greet,
    Saver,
    Session,
    Power,
    Caffeine,
    Night,
    Dnd,
    Lock,
    Suspend,
    Reboot,
    Poweroff,
}

impl Setting {
    pub fn label(self) -> &'static str {
        match self {
            Setting::Mode => "modo",
            Setting::Accent => "acento",
            Setting::Motion => "movimento",
            Setting::Wallpaper => "papel de parede",
            Setting::Prompt => "prompt",
            Setting::Greet => "saudação",
            Setting::Saver => "proteção de tela",
            Setting::Session => "sessão no login",
            Setting::Power => "energia",
            Setting::Caffeine => "modo café",
            Setting::Night => "modo noturno",
            Setting::Dnd => "não perturbe",
            Setting::Lock => "bloquear a tela",
            Setting::Suspend => "suspender",
            Setting::Reboot => "reiniciar",
            Setting::Poweroff => "desligar",
        }
    }

    /// Ações que pedem um segundo Enter.
    pub fn needs_confirm(self) -> bool {
        matches!(self, Setting::Reboot | Setting::Poweroff)
    }

    pub fn is_action(self) -> bool {
        matches!(self, Setting::Lock | Setting::Suspend | Setting::Reboot | Setting::Poweroff)
    }
}

pub struct App {
    pub ctx: Ctx,
    pub theme: Theme,
    pub page: Page,
    pub groups: Vec<Group>,
    pub group_sel: usize,
    pub tips: Vec<Tip>,
    pub tip: usize,
    pub catalog: Catalog,
    pub installed: Vec<bool>,
    pub cat_sel: usize,
    pub app_sel: usize,
    pub steps: Vec<Step>,
    pub info: Info,
    pub settings: Vec<Setting>,
    pub sys_sel: usize,
    pub confirm: Option<Setting>,
    pub power: Option<String>,
    /// Café, noturno e não perturbe (no sandbox os dois primeiros são simulados, sem subir processo).
    pub caffeine: bool,
    pub night: bool,
    pub dnd: bool,
    pub notice: Option<(String, Instant)>,
    pub hour: u32,
    pub motion: Motion,
    pub started: Instant,
    pub page_at: Instant,
    pub quit: bool,
    pub demo: bool,
}

const POWER_PROFILES: [&str; 3] = ["power-saver", "balanced", "performance"];

fn power_label(p: &str) -> &'static str {
    match p {
        "power-saver" => "economia",
        "balanced" => "equilibrado",
        "performance" => "desempenho",
        _ => "?",
    }
}

fn read_power() -> Option<String> {
    let out = std::process::Command::new("powerprofilesctl").arg("get").output().ok()?;
    out.status.success().then(|| String::from_utf8_lossy(&out.stdout).trim().to_string()).filter(|s| !s.is_empty())
}

impl App {
    pub fn new(ctx: Ctx, page: Page, now: Instant) -> Result<Self> {
        let theme = ctx.theme()?;
        let (catalog, warn) = crate::catalog::load(&ctx.paths);
        let power = if ctx.sandboxed { None } else { read_power() };
        let mut settings = vec![
            Setting::Mode,
            Setting::Accent,
            Setting::Motion,
            Setting::Wallpaper,
            Setting::Prompt,
            Setting::Greet,
            Setting::Saver,
            Setting::Session,
        ];
        if power.is_some() {
            settings.push(Setting::Power);
        }
        settings.extend([Setting::Caffeine, Setting::Night, Setting::Dnd]);
        settings.extend([Setting::Lock, Setting::Suspend, Setting::Reboot, Setting::Poweroff]);
        let tips = data::tips();
        // Um cartão por dia: quem abre o guia amanhã vê outra dica.
        let day =
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs() / 86_400);
        let motion = Motion::from_theme(&theme);
        let mut app = App {
            theme,
            page,
            groups: data::groups(),
            group_sel: 0,
            tip: (day as usize) % tips.len(),
            tips,
            installed: Vec::new(),
            catalog,
            cat_sel: 0,
            app_sel: 0,
            steps: Vec::new(),
            info: if ctx.sandboxed { Info::default() } else { sysinfo::gather() },
            settings,
            sys_sel: 0,
            confirm: None,
            power,
            caffeine: false,
            night: false,
            dnd: false,
            notice: warn.map(|w| (w, now)),
            hour: sysinfo::local_hour().unwrap_or(20),
            motion,
            started: now,
            page_at: now,
            quit: false,
            demo: false,
            ctx,
        };
        app.refresh();
        Ok(app)
    }

    /// Relê o que muda fora do guia: o que está instalado, a rede, os primeiros passos.
    pub fn refresh(&mut self) {
        let demo = self.demo;
        self.installed = self
            .catalog
            .tui
            .iter()
            .map(|t| if demo { t.tier == crate::catalog::Tier::Core } else { t.installed() })
            .collect();
        let extras_missing = self
            .catalog
            .tui
            .iter()
            .zip(&self.installed)
            .filter(|(t, ok)| t.tier == crate::catalog::Tier::Extra && !**ok)
            .count();
        let probes = if self.demo {
            Probes { hub_used: true, online: true, aur_helper: true, extras_missing }
        } else if self.ctx.sandboxed {
            Probes::default()
        } else {
            Probes {
                hub_used: self.ctx.paths.state.join("hub-history.toml").exists(),
                online: crate::cmd::status::net().kind != "none",
                aur_helper: crate::sys::is_installed("paru") || crate::sys::is_installed("yay"),
                extras_missing,
            }
        };
        self.steps = data::steps(&self.ctx.state, &probes);
        self.dnd = toggles::dnd_on(&self.ctx);
        if !self.ctx.sandboxed {
            self.caffeine = toggles::CAFFEINE.running(&self.ctx);
            self.night = toggles::NIGHT.running(&self.ctx);
        }
    }

    pub fn say(&mut self, msg: impl Into<String>, now: Instant) {
        self.notice = Some((msg.into(), now));
    }

    pub fn go(&mut self, page: Page, now: Instant) {
        if page != self.page {
            self.page = page;
            self.page_at = now;
            self.confirm = None;
            if page == Page::Apps || page == Page::Inicio {
                self.refresh();
            }
        }
    }

    pub fn step_page(&mut self, delta: isize, now: Instant) {
        let n = Page::ALL.len() as isize;
        let next = (self.page.index() as isize + delta).rem_euclid(n) as usize;
        self.go(Page::ALL[next], now);
    }

    /// As apps (índices do catálogo) da categoria selecionada.
    pub fn apps_in_cat(&self) -> Vec<usize> {
        let cat = CATEGORIES[self.cat_sel].0;
        self.catalog.tui.iter().enumerate().filter(|(_, t)| t.category == cat).map(|(i, _)| i).collect()
    }

    pub fn selected_app(&self) -> Option<usize> {
        self.apps_in_cat().get(self.app_sel).copied()
    }

    fn cycle(&mut self, len: usize, cur: usize, delta: isize) -> usize {
        (cur as isize + delta).rem_euclid(len.max(1) as isize) as usize
    }

    /// Move a seleção da página atual (setas verticais).
    pub fn move_sel(&mut self, delta: isize) {
        self.confirm = None;
        match self.page {
            Page::Atalhos => self.group_sel = self.cycle(self.groups.len(), self.group_sel, delta),
            Page::Apps => {
                let n = self.apps_in_cat().len();
                self.app_sel = self.cycle(n, self.app_sel, delta);
            }
            Page::Sistema => self.sys_sel = self.cycle(self.settings.len(), self.sys_sel, delta),
            Page::Dicas => self.tip = self.cycle(self.tips.len(), self.tip, delta),
            _ => {}
        }
    }

    /// Setas horizontais: muda o valor (sistema), a categoria (apps) ou o cartão (dicas).
    pub fn move_side(&mut self, delta: isize, now: Instant) {
        match self.page {
            Page::Sistema => self.adjust(delta, now),
            Page::Apps => {
                self.cat_sel = self.cycle(CATEGORIES.len(), self.cat_sel, delta);
                self.app_sel = 0;
            }
            Page::Dicas => self.tip = self.cycle(self.tips.len(), self.tip, delta),
            Page::Atalhos => self.group_sel = self.cycle(self.groups.len(), self.group_sel, delta),
            _ => {}
        }
    }

    pub fn setting_value(&self, s: Setting) -> String {
        match s {
            Setting::Mode => if self.theme.mode == Mode::Dark { "escuro" } else { "claro" }.into(),
            Setting::Accent => self.theme.accent_name.clone(),
            Setting::Motion => match self.theme.motion.level {
                MotionLevel::Full => "completo",
                MotionLevel::Reduced => "reduzido",
                MotionLevel::Off => "desligado",
            }
            .into(),
            Setting::Wallpaper => {
                wp::Choice::parse(&self.ctx.state.wallpaper).map_or_else(|_| "?".into(), |c| c.label())
            }
            Setting::Prompt => self.ctx.state.prompt.id().into(),
            Setting::Greet => match self.ctx.state.greet {
                GreetMode::All => "tudo",
                GreetMode::Boot => "só ao ligar",
                GreetMode::Off => "nunca",
            }
            .into(),
            Setting::Saver => match self.ctx.state.saver.as_str() {
                "auto" => "rodízio".to_string(),
                "off" => "desligada".to_string(),
                other => other.to_string(),
            },
            Setting::Session => if self.ctx.state.session { "reabrir" } else { "mesa limpa" }.into(),
            Setting::Power => self.power.as_deref().map_or("indisponível", power_label).into(),
            Setting::Caffeine => state_word(self.caffeine).into(),
            Setting::Night => state_word(self.night).into(),
            Setting::Dnd => state_word(self.dnd).into(),
            _ => "↵".into(),
        }
    }

    fn reload_theme(&mut self) {
        if let Ok(t) = self.ctx.theme() {
            self.motion = Motion::from_theme(&t);
            self.theme = t;
        }
    }

    /// Muda o valor da configuração selecionada: `delta` é +1 ou -1.
    pub fn adjust(&mut self, delta: isize, now: Instant) {
        let Some(&s) = self.settings.get(self.sys_sel) else { return };
        let result: Result<String> = (|| match s {
            Setting::Mode => {
                let m = self.theme.mode.toggled();
                theme_cmd::set_quiet(&mut self.ctx, Some(m), None, None)?;
                Ok(format!("modo {}", self.setting_value(Setting::Mode)))
            }
            Setting::Accent => {
                let names: Vec<String> = self.theme.accents.iter().map(|a| a.name.clone()).collect();
                let cur = names.iter().position(|n| *n == self.theme.accent_name).unwrap_or(0);
                let next = names[(cur as isize + delta).rem_euclid(names.len() as isize) as usize].clone();
                theme_cmd::set_quiet(&mut self.ctx, None, Some(next.clone()), None)?;
                Ok(format!("acento {next}"))
            }
            Setting::Motion => {
                let l = self.theme.motion.level;
                let next = if delta > 0 { l.cycled() } else { l.cycled().cycled() };
                theme_cmd::set_quiet(&mut self.ctx, None, None, Some(next))?;
                Ok(format!("movimento {next}"))
            }
            Setting::Wallpaper => Ok(format!("papel de parede: {}", wp::step_quiet(&mut self.ctx, delta)?.label())),
            Setting::Prompt => {
                let next = self.ctx.state.prompt.step(delta);
                crate::cmd::prompt::set_quiet(&mut self.ctx, next)?;
                Ok(format!("prompt {}: {}", next.id(), next.blurb()))
            }
            Setting::Greet => {
                self.ctx.state.greet = self.ctx.state.greet.step(delta);
                self.ctx.save_state()?;
                Ok(format!("saudação do terminal: {}", crate::cmd::greet::describe(self.ctx.state.greet)))
            }
            Setting::Saver => {
                let (cat, _) = crate::catalog::load(&self.ctx.paths);
                let all = crate::cmd::saver::valid_settings(&cat);
                let cur = all.iter().position(|v| *v == self.ctx.state.saver).unwrap_or(0) as isize;
                self.ctx.state.saver = all[(cur + delta).rem_euclid(all.len() as isize) as usize].clone();
                self.ctx.save_state()?;
                Ok(format!("proteção de tela: {}", crate::cmd::saver::describe(&self.ctx.state.saver)))
            }
            Setting::Session => {
                self.ctx.state.session = !self.ctx.state.session;
                self.ctx.save_state()?;
                Ok(if self.ctx.state.session {
                    "as janelas da última sessão voltam no login".into()
                } else {
                    "o login começa com a mesa limpa".into()
                })
            }
            Setting::Power => {
                let Some(cur) = self.power.clone() else { bail!("powerprofilesctl indisponível") };
                let i = POWER_PROFILES.iter().position(|p| *p == cur).unwrap_or(1) as isize;
                let next = POWER_PROFILES[(i + delta).rem_euclid(3) as usize];
                if !self.ctx.sandboxed {
                    let ok = std::process::Command::new("powerprofilesctl").args(["set", next]).status()?.success();
                    if !ok {
                        bail!("o powerprofilesctl recusou {next}");
                    }
                }
                self.power = Some(next.to_string());
                Ok(format!("energia: {}", power_label(next)))
            }
            Setting::Caffeine => {
                if !self.ctx.sandboxed {
                    self.caffeine = toggles::caffeine(&self.ctx, Switch::Toggle)?;
                } else {
                    self.caffeine = !self.caffeine;
                }
                Ok(format!("café {}", state_word(self.caffeine)))
            }
            Setting::Night => {
                if !self.ctx.sandboxed {
                    self.night = toggles::night(&self.ctx, Switch::Toggle, None)?;
                } else {
                    self.night = !self.night;
                }
                Ok(format!("noturno {}", state_word(self.night)))
            }
            Setting::Dnd => {
                self.dnd = toggles::dnd(&self.ctx, Switch::Toggle)?;
                Ok(format!("não perturbe {}", state_word(self.dnd)))
            }
            _ => Ok(String::new()),
        })();
        match result {
            Ok(m) if !m.is_empty() => {
                self.reload_theme();
                self.say(m, now);
            }
            Ok(_) => {}
            Err(e) => self.say(format!("não consegui: {e:#}"), now),
        }
    }

    fn spawn_shell(&mut self, cmd: &str, now: Instant) {
        if self.ctx.sandboxed {
            self.say("modo simulação: nada foi executado", now);
            return;
        }
        let argv = ["setsid", "-f", "sh", "-c", cmd].map(String::from);
        if let Err(e) = exec::spawn(&argv) {
            self.say(format!("não consegui: {e:#}"), now);
        }
    }

    fn open_action(&mut self, action: &Action, what: &str, now: Instant) {
        if self.ctx.sandboxed {
            self.say(format!("modo simulação: abriria {what}"), now);
            return;
        }
        let Some(argv) = exec::plan(action, &exec::Env::detect()) else { return };
        match exec::spawn(&argv) {
            Ok(()) => self.say(format!("abrindo {what}"), now),
            Err(e) => self.say(format!("não consegui abrir {what}: {e:#}"), now),
        }
    }

    /// Enter: o que cada página entende por "usar isto".
    pub fn activate(&mut self, now: Instant) {
        match self.page {
            Page::Apps => {
                let Some(i) = self.selected_app() else { return };
                let t = self.catalog.tui[i].clone();
                if self.installed[i] {
                    let a = Action::Tui {
                        id: t.id.clone(),
                        argv: t.launch_argv(&self.theme),
                        float: t.float,
                        hold: t.hold,
                    };
                    self.open_action(&a, &t.name, now);
                } else if t.pkg.is_empty() {
                    self.say(format!("{} não tem pacote para instalar", t.name), now);
                } else {
                    let a = Action::Tui { id: "install".into(), argv: t.install_argv(), float: true, hold: true };
                    self.open_action(&a, &format!("a instalação de {}", t.name), now);
                }
            }
            Page::Sistema => {
                let Some(&s) = self.settings.get(self.sys_sel) else { return };
                match s {
                    Setting::Wallpaper => {
                        let a = Action::Tui {
                            id: "wallpaper".into(),
                            argv: vec!["clios".into(), "wallpaper".into()],
                            float: true,
                            hold: false,
                        };
                        self.open_action(&a, "o seletor de papel de parede", now);
                    }
                    s if s.is_action() => {
                        if s.needs_confirm() && self.confirm != Some(s) {
                            self.confirm = Some(s);
                            self.say(format!("{}: aperte Enter de novo para confirmar", s.label()), now);
                            return;
                        }
                        self.confirm = None;
                        let cmd = match s {
                            Setting::Lock => "pidof hyprlock || hyprlock",
                            Setting::Suspend => "systemctl suspend",
                            Setting::Reboot => "systemctl reboot",
                            _ => "systemctl poweroff",
                        };
                        self.spawn_shell(cmd, now);
                    }
                    _ => self.adjust(1, now),
                }
            }
            Page::Dicas => self.tip = self.cycle(self.tips.len(), self.tip, 1),
            Page::Inicio => self.go(Page::Atalhos, now),
            _ => {}
        }
    }

    pub fn on_key(&mut self, k: ratatui::crossterm::event::KeyEvent, now: Instant) {
        if k.kind == KeyEventKind::Release {
            return;
        }
        match (k.code, k.modifiers) {
            (KeyCode::Char('q') | KeyCode::Esc, _) | (KeyCode::Char('c'), KeyModifiers::CONTROL) => self.quit = true,
            (KeyCode::Tab, _) => self.step_page(1, now),
            (KeyCode::BackTab, _) => self.step_page(-1, now),
            (KeyCode::Char(c @ '1'..='6'), _) => self.go(Page::ALL[c as usize - '1' as usize], now),
            (KeyCode::Char('['), _) => self.step_page(-1, now),
            (KeyCode::Char(']'), _) => self.step_page(1, now),
            (KeyCode::Down | KeyCode::Char('j'), _) => self.move_sel(1),
            (KeyCode::Up | KeyCode::Char('k'), _) => self.move_sel(-1),
            (KeyCode::Right | KeyCode::Char('l'), _) => self.move_side(1, now),
            (KeyCode::Left | KeyCode::Char('h'), _) => self.move_side(-1, now),
            (KeyCode::Enter | KeyCode::Char(' '), _) => self.activate(now),
            (KeyCode::Char('r'), _) => {
                self.refresh();
                self.say("atualizado", now);
            }
            _ => {}
        }
    }

    pub fn on_mouse(&mut self, m: ratatui::crossterm::event::MouseEvent, area: Rect, now: Instant) {
        match m.kind {
            MouseEventKind::ScrollDown => self.move_sel(1),
            MouseEventKind::ScrollUp => self.move_sel(-1),
            MouseEventKind::Down(MouseButton::Left) => {
                if let Some(p) = pages::tab_at(self, m.column, m.row) {
                    self.go(p, now);
                } else if pages::click(self, area, m.column, m.row) {
                    self.activate(now);
                }
            }
            _ => {}
        }
    }

    /// Há algo se mexendo? (A entrada escalonada e a piscada do cursor.)
    pub fn animating(&self, now: Instant) -> bool {
        self.theme.motion.enabled && now.duration_since(self.page_at) < Duration::from_millis(900)
    }

    pub fn cursor_on(&self, now: Instant) -> bool {
        // Pisca a cada ~530 ms, como o cursor de um terminal. Sem movimento, fica aceso.
        !self.theme.motion.enabled || (now.duration_since(self.started).as_millis() / 530) % 2 == 0
    }

    pub fn next_wake(&self, now: Instant) -> Duration {
        let phase = now.duration_since(self.started).as_millis() % 530;
        Duration::from_millis((530 - phase) as u64 + 1)
    }
}

pub struct Options {
    pub page: Option<String>,
    pub first_run: bool,
    pub snapshot: Option<String>,
    pub select: Option<usize>,
    /// Para a documentação: o que vem de fábrica, com os primeiros passos adiantados.
    pub demo: bool,
}

fn seen_marker(ctx: &Ctx) -> std::path::PathBuf {
    ctx.paths.state.join("welcome-seen")
}

pub fn run(ctx: Ctx, opts: Options) -> Result<()> {
    let page = opts.page.as_deref().map(Page::parse).transpose()?.unwrap_or(Page::Inicio);

    if opts.first_run {
        // O autostart chama isto em todo login: só abre a janela se o guia nunca foi visto.
        if seen_marker(&ctx).exists() {
            return Ok(());
        }
        let action = Action::Tui {
            id: "welcome".into(),
            argv: vec!["clios".into(), "welcome".into()],
            float: true,
            hold: false,
        };
        if let Some(argv) = exec::plan(&action, &exec::Env::detect()) {
            exec::spawn(&argv)?;
        }
        return Ok(());
    }

    let now = Instant::now();
    let mut app = App::new(ctx, page, now)?;
    if opts.demo {
        app.demo = true;
        app.refresh();
    }
    if let Some(i) = opts.select {
        pages::set_sel(&mut app, i);
    }

    if let Some(size) = opts.snapshot {
        let (w, h) = size
            .split_once('x')
            .and_then(|(w, h)| Some((w.parse::<u16>().ok()?, h.parse::<u16>().ok()?)))
            .ok_or_else(|| anyhow::anyhow!("tamanho inválido {size:?}, esperava LARGURAxALTURA (ex.: 104x32)"))?;
        let mut buf = Buffer::empty(Rect::new(0, 0, w, h));
        // O quadro é tirado com a entrada já terminada.
        // 5,3 s: a entrada terminou e o cursor da marca está aceso.
        pages::render(&mut buf, &app, now + Duration::from_millis(5300));
        print!("{}", crate::ui::buffer_ansi(&buf));
        return Ok(());
    }

    let mut terminal = ratatui::init();
    let _ = execute!(io::stdout(), EnableMouseCapture);
    let result = event_loop(&mut terminal, &mut app);
    let _ = execute!(io::stdout(), DisableMouseCapture);
    ratatui::restore();
    // Quem fechou o guia já o viu: o autostart não abre de novo.
    let _ = std::fs::create_dir_all(&app.ctx.paths.state).and_then(|()| std::fs::write(seen_marker(&app.ctx), "1\n"));
    result
}

fn event_loop(terminal: &mut ratatui::DefaultTerminal, app: &mut App) -> Result<()> {
    loop {
        let now = Instant::now();
        if app.notice.as_ref().is_some_and(|(_, t)| now.duration_since(*t) > Duration::from_millis(3500)) {
            app.notice = None;
        }
        let size = terminal.size()?;
        let area = Rect::new(0, 0, size.width, size.height);
        terminal.draw(|f| pages::render(f.buffer_mut(), app, now))?;
        let timeout = if app.animating(now) { Duration::from_millis(16) } else { app.next_wake(now) };
        if !event::poll(timeout)? {
            continue;
        }
        loop {
            match event::read()? {
                Event::Key(k) => app.on_key(k, Instant::now()),
                Event::Mouse(m) => app.on_mouse(m, area, Instant::now()),
                _ => {}
            }
            if app.quit {
                return Ok(());
            }
            if !event::poll(Duration::ZERO)? {
                break;
            }
        }
    }
}
