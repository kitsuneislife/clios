//! O hub: lançador e paleta de comandos que roda dentro de um terminal.
//!
//! Abre numa janela flutuante do foot (classe `clios.hub`), pesquisa apps, TUIs, ações,
//! janelas, atalhos e o histórico do clipboard, e some. Uma janela de terminal, não um
//! programa gráfico à parte.

pub mod anim;
pub mod app;
pub mod desktop;
pub mod exec;
pub mod items;
pub mod search;
pub mod sources;
pub mod ui;

use std::io;
use std::time::{Duration, Instant};

use anyhow::Result;
use ratatui::DefaultTerminal;
use ratatui::crossterm::event::{self, DisableMouseCapture, EnableMouseCapture, Event};
use ratatui::crossterm::execute;
use ratatui::layout::Rect;

use crate::ctx::Ctx;
use app::{App, Flow};
use search::History;

pub struct Options {
    /// Imprime os resultados da consulta e sai (sem terminal interativo).
    pub dump: Option<String>,
    /// Renderiza um quadro em ANSI e sai: `LARGURAxALTURA`. Para docs e capturas.
    pub snapshot: Option<String>,
    pub query: String,
    /// Em que instante da animação (ms após abrir) o quadro é tirado.
    pub at_ms: u64,
}

/// `clios open <id>`: abre uma entrada do catálogo (ou o próprio hub) numa janela de terminal.
/// Os atalhos do Hyprland chamam isto, então o catálogo é a única fonte dos comandos.
pub fn open(ctx: &Ctx, id: &str, args: &[String]) -> Result<()> {
    let env = exec::Env::detect();
    let argv = if id == "hub" {
        exec::plan_hub(&env, &args.join(" "))
    } else {
        let (catalog, _) = sources::load_catalog(&ctx.paths);
        let Some(t) = catalog.tui.iter().find(|t| t.id == id) else {
            let ids: Vec<&str> = std::iter::once("hub").chain(catalog.tui.iter().map(|t| t.id.as_str())).collect();
            anyhow::bail!("não há entrada {id:?} no catálogo. Disponíveis: {}", ids.join(", "));
        };
        let action = items::Action::Tui { id: t.id.clone(), argv: t.cmd.clone(), float: t.float, hold: t.hold };
        exec::plan(&action, &env).expect("Tui sempre tem plano")
    };
    exec::spawn(&argv)
}

pub fn run(ctx: &Ctx, opts: Options) -> Result<()> {
    let theme = ctx.theme()?;
    let (catalog, warning) = sources::load_catalog(&ctx.paths);
    let items = sources::static_items(&catalog, &theme);
    let history_path = ctx.paths.state.join("hub-history.toml");
    let mut app = App::new(theme, items, History::load(&history_path), Instant::now());
    app.notice = warning;
    if !opts.query.is_empty() && opts.snapshot.is_none() {
        app.set_query(&opts.query, Instant::now());
    }

    if let Some(query) = opts.dump {
        app.set_query(&query, Instant::now());
        for hit in app.hits.iter().take(25) {
            let item = &app.items[hit.index];
            println!("{}\t{}\t{}", item.kind.label(), item.title, item.hint);
        }
        return Ok(());
    }

    if let Some(size) = opts.snapshot {
        return snapshot(app, &size, &opts.query, opts.at_ms);
    }

    let chosen = interactive(&mut app)?;
    let Some(idx) = chosen else { return Ok(()) };

    // O terminal já foi restaurado; agora abrimos a coisa escolhida e deixamos o hub fechar.
    let item_title = app.items[idx].title.clone();
    if let Some(argv) = exec::plan(&app.items[idx].action, &exec::Env::detect()) {
        app.record_use(idx);
        let _ = app.history.save(&history_path); // histórico é conveniência; falhar não pode atrapalhar
        if let Err(e) = exec::spawn(&argv) {
            let msg = format!("não consegui abrir {item_title}: {e:#}");
            eprintln!("{msg}");
            let _ =
                std::process::Command::new("notify-send").args(["-u", "critical", "-a", "clios", "hub", &msg]).status();
            return Err(e);
        }
    }
    Ok(())
}

/// Um quadro do hub como texto ANSI de 24 bits.
fn snapshot(mut app: App, size: &str, query: &str, at_ms: u64) -> Result<()> {
    use ratatui::buffer::Buffer;
    use ratatui::style::{Color, Modifier};
    use std::fmt::Write as _;

    let (w, h) = size
        .split_once('x')
        .and_then(|(w, h)| Some((w.parse::<u16>().ok()?, h.parse::<u16>().ok()?)))
        .ok_or_else(|| anyhow::anyhow!("tamanho inválido {size:?}, esperava LARGURAxALTURA (ex.: 90x26)"))?;
    let t0 = Instant::now();
    app.reveal_from = t0;
    app.last_key = t0;
    if !query.is_empty() {
        app.set_query(query, t0);
        app.reveal_from = t0;
    }
    app.set_view_rows(usize::from(ui::layout(Rect::new(0, 0, w, h)).list_rows));
    let now = t0 + Duration::from_millis(at_ms);
    let mut buf = Buffer::empty(Rect::new(0, 0, w, h));
    ui::render(&mut buf, &app, now);

    let mut out = String::new();
    for y in 0..h {
        for x in 0..w {
            let c = &buf[(x, y)];
            let rgb = |c: Color| match c {
                Color::Rgb(r, g, b) => (r, g, b),
                _ => (128, 128, 128),
            };
            let (fr, fg, fb) = rgb(c.fg);
            let (br, bg, bb) = rgb(c.bg);
            let bold = if c.modifier.contains(Modifier::BOLD) { "0;1;" } else { "0;" };
            let _ = write!(out, "\x1b[{bold}38;2;{fr};{fg};{fb};48;2;{br};{bg};{bb}m{}", c.symbol());
        }
        out.push_str("\x1b[0m\n");
    }
    print!("{out}");
    Ok(())
}

struct MouseGuard;

impl MouseGuard {
    fn enable() -> Self {
        let _ = execute!(io::stdout(), EnableMouseCapture);
        MouseGuard
    }
}

impl Drop for MouseGuard {
    fn drop(&mut self) {
        let _ = execute!(io::stdout(), DisableMouseCapture);
    }
}

fn interactive(app: &mut App) -> Result<Option<usize>> {
    let mut terminal = ratatui::init();
    let guard = MouseGuard::enable();
    let result = event_loop(&mut terminal, app);
    drop(guard);
    ratatui::restore();
    result
}

fn event_loop(terminal: &mut DefaultTerminal, app: &mut App) -> Result<Option<usize>> {
    loop {
        let now = Instant::now();
        let size = terminal.size()?;
        let lay = ui::layout(Rect::new(0, 0, size.width, size.height));
        app.set_view_rows(usize::from(lay.list_rows));
        terminal.draw(|f| ui::render(f.buffer_mut(), app, now))?;

        // Quadros rápidos só enquanto algo se move; parado, dormimos até a próxima piscada.
        let timeout = if app.animating(now) { Duration::from_millis(16) } else { app.next_wake(now) };
        if !event::poll(timeout)? {
            continue;
        }
        // Esvazia a fila antes de redesenhar, para não atrasar quem digita rápido.
        loop {
            let flow = match event::read()? {
                Event::Key(k) => app.on_key(k, Instant::now()),
                Event::Mouse(m) => app.on_mouse(m, usize::from(lay.list_top), Instant::now()),
                _ => Flow::Continue,
            };
            match flow {
                Flow::Quit => return Ok(None),
                Flow::Run(i) => return Ok(Some(i)),
                Flow::Continue => {}
            }
            if !event::poll(Duration::ZERO)? {
                break;
            }
        }
    }
}
