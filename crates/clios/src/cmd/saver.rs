//! `clios saver`: a proteção de tela do CLIOS. A cena padrão é a marca deslizando pela tela: cada vez que
//! bate numa borda, o cursor dela troca para o próximo acento. As outras cenas são os brinquedos do
//! catálogo (bonsai, aquário, tubulações...) que estiverem instalados, na cor mais perto do seu acento.
//!
//! `clios saver` abre uma janela de terminal em tela cheia (é o que o hypridle chama);
//! `clios saver --run` é o que roda dentro dela. O hypridle fecha a janela ao primeiro sinal de vida
//! (`on-resume`); aberta à mão, `q` sai dos brinquedos e qualquer tecla sai da marca.

use std::io;
use std::time::{Duration, Instant};

use anyhow::{Result, bail};
use clios_core::{Rgb, Theme};
use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{self, DisableMouseCapture, EnableMouseCapture, Event};
use ratatui::crossterm::execute;

use crate::art;
use crate::catalog;
use crate::ctx::Ctx;
use crate::hub::exec;
use crate::hub::items::Action;
use crate::toys;

/// O que o estado `saver` pede, já resolvido contra o que está instalado.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Pick {
    /// A proteção de tela está desligada.
    Off,
    /// A marca deslizando.
    Mark,
    /// Um brinquedo do catálogo, pelo id.
    Toy(String),
}

/// Escolhe a cena. `setting` é `auto`, `off`, `marca` ou o id de um brinquedo; `installed` são os ids que
/// dá para rodar agora; `seed` desempata o rodízio (o relógio, na prática, e um número fixo nos testes).
/// Um brinquedo pedido e não instalado cai para a marca: a tela nunca fica sem proteção por causa disso.
pub fn pick(setting: &str, installed: &[&str], seed: usize) -> Pick {
    match setting {
        "off" => Pick::Off,
        "marca" | "mark" => Pick::Mark,
        "auto" | "" => {
            // A marca entra no rodízio como uma cena igual às outras.
            let n = installed.len() + 1;
            match seed % n {
                0 => Pick::Mark,
                i => Pick::Toy(installed[i - 1].to_string()),
            }
        }
        id if installed.contains(&id) => Pick::Toy(id.to_string()),
        _ => Pick::Mark,
    }
}

/// Valores aceitos em `clios saver set`: os fixos e os ids dos brinquedos do catálogo.
pub fn valid_settings(cat: &catalog::Catalog) -> Vec<String> {
    let mut v: Vec<String> = ["auto", "marca", "off"].map(String::from).to_vec();
    v.extend(cat.tui.iter().filter(|t| !t.saver.is_empty()).map(|t| t.id.clone()));
    v
}

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

/// Abre a proteção de tela numa janela em tela cheia. `scene` força uma cena (e ignora o `off`).
pub fn launch(ctx: &Ctx, scene: Option<&str>) -> Result<()> {
    if scene.is_none() && ctx.state.saver == "off" {
        return Ok(());
    }
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
        argv: {
            let mut a = vec![env.exe.clone(), "saver".into(), "--run".into()];
            if let Some(sc) = scene {
                a.extend(["--scene".into(), sc.into()]);
            }
            a
        },
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

/// `clios saver set`: escolhe a cena (ou o rodízio, ou nada).
pub fn set(ctx: &mut Ctx, value: &str) -> Result<()> {
    let (cat, _) = catalog::load(&ctx.paths);
    let valid = valid_settings(&cat);
    let value = if value == "mark" { "marca" } else { value };
    if !valid.iter().any(|v| v == value) {
        bail!("{value:?} não é uma cena. Use: {}", valid.join(", "));
    }
    ctx.state.saver = value.to_string();
    ctx.save_state()?;
    println!("proteção de tela: {}", describe(value));
    Ok(())
}

pub fn describe(setting: &str) -> &'static str {
    match setting {
        "auto" => "rodízio (a marca e os brinquedos instalados)",
        "marca" => "a marca deslizando",
        "off" => "desligada",
        _ => "um brinquedo só",
    }
}

/// `clios saver list`: as cenas, marcando a escolhida e o que está instalado.
pub fn list(ctx: &Ctx) -> Result<()> {
    let (cat, _) = catalog::load(&ctx.paths);
    let cur = ctx.state.saver.as_str();
    let mark = |id: &str| if id == cur { "  ←" } else { "" };
    println!("{:<12} {}{}", "auto", describe("auto"), mark("auto"));
    println!("{:<12} {}{}", "marca", describe("marca"), mark("marca"));
    for t in cat.tui.iter().filter(|t| !t.saver.is_empty()) {
        let state = if t.installed() { String::new() } else { format!("  (falta: clios apps install {})", t.id) };
        println!("{:<12} {}{}{state}", t.id, t.desc, mark(&t.id));
    }
    println!("{:<12} {}{}", "off", describe("off"), mark("off"));
    Ok(())
}

/// Fecha a proteção de tela aberta (é o que o hypridle faz ao primeiro sinal de vida).
pub fn stop() {
    let _ = std::process::Command::new("pkill").args(["-f", "clios saver --run"]).status();
}

/// Roda aqui, no terminal atual. Sem `scene`, vale o que o usuário escolheu.
pub fn run(ctx: &Ctx, scene: Option<&str>) -> Result<()> {
    let theme = ctx.theme()?;
    let (cat, _) = catalog::load(&ctx.paths);
    let installed = toys::installed_scenes(&cat.tui);
    let ids: Vec<&str> = installed.iter().map(|t| t.id.as_str()).collect();
    let seed =
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.subsec_nanos() as usize);
    let setting = scene.unwrap_or(&ctx.state.saver);
    match pick(setting, &ids, seed) {
        Pick::Off => return Ok(()),
        Pick::Toy(id) => {
            if let Some(argv) = installed.iter().find(|t| t.id == id).and_then(|t| t.scene_argv(&theme)) {
                // Se o brinquedo não abrir, a marca assume: tela preta seria pior.
                if run_toy(&argv) {
                    return Ok(());
                }
            }
        }
        Pick::Mark => {}
    }
    run_mark(&theme)
}

/// Roda o brinquedo no terminal atual e espera. `false` se não conseguiu nem começar.
fn run_toy(argv: &[String]) -> bool {
    let Some((prog, args)) = argv.split_first() else { return false };
    std::process::Command::new(prog).args(args).status().is_ok()
}

fn run_mark(theme: &Theme) -> Result<()> {
    let theme = theme.clone();
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
    fn pick_follows_the_setting_and_falls_back_to_the_mark() {
        let have = ["bonsai", "pipes"];
        assert_eq!(pick("off", &have, 0), Pick::Off);
        assert_eq!(pick("marca", &have, 1), Pick::Mark);
        assert_eq!(pick("pipes", &have, 0), Pick::Toy("pipes".into()));
        assert_eq!(pick("aquarium", &have, 0), Pick::Mark, "pedido e não instalado: a marca assume");
        assert_eq!(pick("lixo", &[], 0), Pick::Mark);
    }

    #[test]
    fn auto_rotates_through_the_mark_and_every_installed_toy() {
        let have = ["bonsai", "pipes"];
        let seen: Vec<Pick> = (0..3).map(|s| pick("auto", &have, s)).collect();
        assert_eq!(seen, [Pick::Mark, Pick::Toy("bonsai".into()), Pick::Toy("pipes".into())]);
        assert_eq!(pick("auto", &have, 3), Pick::Mark, "dá a volta");
        assert_eq!(pick("auto", &[], 7), Pick::Mark, "sem brinquedos, só a marca");
    }

    #[test]
    fn every_toy_of_the_catalog_is_a_valid_setting() {
        let v = valid_settings(&catalog::builtin());
        for want in ["auto", "marca", "off", "bonsai", "aquarium", "pipes", "sakura", "lava", "clock", "matrix-rain"] {
            assert!(v.iter().any(|x| x == want), "{want} deveria valer: {v:?}");
        }
        assert!(!v.iter().any(|x| x == "visualizer"), "o cava não faz sentido sem música: fica de fora");
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
