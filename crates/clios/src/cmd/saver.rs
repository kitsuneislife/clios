//! `clios saver`: a proteção de tela do CLIOS. A marca desliza pela tela, e cada vez que bate numa borda
//! o cursor dela troca para o próximo acento. Qualquer tecla ou movimento do mouse fecha.
//!
//! `clios saver` abre uma janela de terminal em tela cheia (é o que o hypridle chama);
//! `clios saver --run` é o que roda dentro dela.

use std::io;
use std::time::{Duration, Instant};

use anyhow::Result;
use clios_core::{Rgb, Theme};
use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{self, DisableMouseCapture, EnableMouseCapture, Event};
use ratatui::crossterm::execute;

use crate::art;
use crate::ctx::Ctx;
use crate::hub::exec;
use crate::hub::items::Action;

/// Altura da marca, em linhas de terminal (a largura é o dobro).
const ROWS: u16 = 8;

/// A posição e a direção da marca. A física é pura e testável.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Drift {
    pub x: i32,
    pub y: i32,
    pub dx: i32,
    pub dy: i32,
}

impl Drift {
    /// Avança um passo dentro de `w` x `h`. Devolve o novo estado e se bateu em alguma borda.
    pub fn step(self, w: i32, h: i32) -> (Drift, bool) {
        let (mw, mh) = (i32::from(ROWS) * 2, i32::from(ROWS));
        let (max_x, max_y) = ((w - mw).max(0), (h - mh).max(0));
        let mut d = self;
        d.x += d.dx;
        d.y += d.dy;
        let mut hit = false;
        if d.x <= 0 || d.x >= max_x {
            d.dx = -d.dx;
            d.x = d.x.clamp(0, max_x);
            hit = true;
        }
        if d.y <= 0 || d.y >= max_y {
            d.dy = -d.dy;
            d.y = d.y.clamp(0, max_y);
            hit = true;
        }
        (d, hit)
    }
}

/// O próximo acento, na ordem do tema.
pub fn next_accent(theme: &Theme, current: usize) -> usize {
    (current + 1) % theme.accents.len().max(1)
}

pub fn render(buf: &mut Buffer, theme: &Theme, d: &Drift, accent: Rgb, cursor_on: bool) {
    let area = buf.area;
    let bg = theme.c.bg;
    for y in 0..area.height {
        for x in 0..area.width {
            buf[(x, y)].set_char(' ').set_bg(ratatui::style::Color::Rgb(bg.r, bg.g, bg.b));
        }
    }
    let mut t = theme.clone();
    t.c.accent = accent;
    let cells = art::mark_cells(&t, u32::from(ROWS), cursor_on, bg);
    let w = usize::from(ROWS) * 2;
    for (i, c) in cells.iter().enumerate() {
        let (x, y) = (d.x + (i % w) as i32, d.y + (i / w) as i32);
        if x < 0 || y < 0 || x >= i32::from(area.width) || y >= i32::from(area.height) {
            continue;
        }
        let cell = &mut buf[(x as u16, y as u16)];
        cell.set_char('▀');
        cell.set_fg(ratatui::style::Color::Rgb(c.top.r, c.top.g, c.top.b));
        cell.set_bg(ratatui::style::Color::Rgb(c.bottom.r, c.bottom.g, c.bottom.b));
    }
}

/// Abre a proteção de tela numa janela em tela cheia.
pub fn launch() -> Result<()> {
    // já tem uma rodando? não empilha outra
    let running = std::process::Command::new("pgrep")
        .args(["-f", "clios saver --run"])
        .output()
        .is_ok_and(|o| !o.stdout.is_empty());
    if running {
        return Ok(());
    }
    let env = exec::Env::detect();
    let action = Action::Tui {
        id: "saver".into(),
        argv: vec![env.exe.clone(), "saver".into(), "--run".into()],
        float: false,
        hold: false,
    };
    let Some(mut argv) = exec::plan(&action, &env) else { return Ok(()) };
    // classe própria e tela cheia (a regra de janela do Hyprland cuida do resto)
    if let Some(p) = argv.iter().position(|a| a == "clios.tui.saver") {
        argv[p] = "clios.saver".into();
    }
    if let Some(p) = argv.iter().position(|a| a == "footclient") {
        argv.insert(p + 1, "-F".into());
    }
    exec::spawn(&argv)
}

pub fn run(ctx: &Ctx) -> Result<()> {
    let theme = ctx.theme()?;
    let mut terminal = ratatui::init();
    let _ = execute!(io::stdout(), EnableMouseCapture);
    let result = (|| -> Result<()> {
        let size = terminal.size()?;
        let mut d = Drift { x: i32::from(size.width) / 3, y: i32::from(size.height) / 4, dx: 1, dy: 1 };
        let mut accent_i = theme.accents.iter().position(|a| a.name == theme.accent_name).unwrap_or(0);
        let start = Instant::now();
        let mut last_step = start;
        let mut armed = false;
        loop {
            let now = Instant::now();
            let size = terminal.size()?;
            if now.duration_since(last_step) >= Duration::from_millis(70) {
                let (nd, hit) = d.step(i32::from(size.width), i32::from(size.height));
                d = nd;
                if hit {
                    accent_i = next_accent(&theme, accent_i);
                }
                last_step = now;
            }
            let accent = theme.accents[accent_i].color;
            let blink = (now.duration_since(start).as_millis() / 530) % 2 == 0;
            terminal.draw(|f| render(f.buffer_mut(), &theme, &d, accent, blink))?;
            if event::poll(Duration::from_millis(35))? {
                let ev = event::read()?;
                // O mouse costuma mandar um evento logo ao abrir; só fecha depois de um instante.
                if matches!(ev, Event::Key(_)) || (armed && matches!(ev, Event::Mouse(_))) {
                    return Ok(());
                }
            }
            armed = armed || now.duration_since(start) > Duration::from_millis(600);
        }
    })();
    let _ = execute!(io::stdout(), DisableMouseCapture);
    ratatui::restore();
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use clios_core::{Mode, MotionLevel, Tokens};
    use ratatui::layout::Rect;

    fn theme() -> Theme {
        Theme::resolve(&Tokens::builtin(), Mode::Dark, "ember", MotionLevel::Full).unwrap()
    }

    #[test]
    fn drift_moves_diagonally_and_bounces_off_every_edge() {
        let (w, h) = (60, 20);
        let mut d = Drift { x: 5, y: 5, dx: 1, dy: 1 };
        let mut hits = 0;
        for _ in 0..2000 {
            let (nd, hit) = d.step(w, h);
            d = nd;
            hits += i32::from(hit);
            assert!((0..=w - 16).contains(&d.x) && (0..=h - 8).contains(&d.y), "saiu da tela: {d:?}");
        }
        assert!(hits > 20, "tem que ficar quicando, só bateu {hits} vezes");
    }

    #[test]
    fn a_hit_flips_only_the_axis_that_hit() {
        let (d, hit) = Drift { x: 43, y: 5, dx: 1, dy: 1 }.step(60, 30);
        assert!(hit);
        assert_eq!((d.x, d.dx, d.dy), (44, -1, 1), "bateu à direita: inverte o x, mantém o y");
        let (d, hit) = Drift { x: 10, y: 21, dx: 1, dy: 1 }.step(60, 30);
        assert!(hit);
        assert_eq!((d.y, d.dx, d.dy), (22, 1, -1), "bateu embaixo: inverte o y");
    }

    #[test]
    fn a_screen_smaller_than_the_mark_does_not_panic() {
        let mut d = Drift { x: 0, y: 0, dx: 1, dy: 1 };
        for _ in 0..50 {
            d = d.step(5, 3).0;
        }
        assert_eq!((d.x, d.y), (0, 0));
    }

    #[test]
    fn accents_cycle() {
        let t = theme();
        let n = t.accents.len();
        assert_eq!(next_accent(&t, n - 1), 0);
        assert_eq!(next_accent(&t, 0), 1);
    }

    #[test]
    fn render_draws_the_mark_where_it_is_and_clips_at_the_edge() {
        let t = theme();
        let mut buf = Buffer::empty(Rect::new(0, 0, 60, 20));
        render(&mut buf, &t, &Drift { x: 10, y: 4, dx: 1, dy: 1 }, t.c.accent, true);
        assert_eq!(buf[(10, 4)].symbol(), "▀", "o canto da marca");
        assert_eq!(buf[(2, 2)].symbol(), " ", "fora da marca é fundo");
        // cursor aceso no acento escolhido
        let acc = ratatui::style::Color::Rgb(t.c.accent.r, t.c.accent.g, t.c.accent.b);
        let has_accent = (0..60).any(|x| (0..20).any(|y| buf[(x, y)].fg == acc || buf[(x, y)].bg == acc));
        assert!(has_accent);
        // meio fora da tela: não pode entrar em pânico
        let mut buf = Buffer::empty(Rect::new(0, 0, 60, 20));
        render(&mut buf, &t, &Drift { x: 55, y: 17, dx: 1, dy: 1 }, t.c.accent, false);
    }
}
