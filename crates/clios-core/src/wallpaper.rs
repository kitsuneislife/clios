//! Papéis de parede procedurais.
//!
//! Cada estilo é uma função (x, y) → cor, desenhada só com as cores do tema: mudou o modo ou o acento,
//! o papel de parede muda junto, sem arquivo de imagem no repositório. Tudo é relativo à altura
//! (1080 px = escala 1), então o resultado é o mesmo em 1080p, 1440p ou 4K.
//!
//! Os contornos usam distância com sinal e cobertura de meio pixel (borda suave sem supersampling),
//! e um ruído de ±0,5 nível tira o degradê em faixas.

use std::str::FromStr;

use anyhow::{Context, Result, bail};

use crate::color::Rgb;
use crate::theme::Colors;

type V3 = [f32; 3];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Style {
    Solido,
    Grade,
    Aneis,
    Linhas,
    Blocos,
    CGigante,
    Diagonais,
    Brilho,
}

impl Style {
    pub const ALL: [Style; 8] = [
        Style::Grade,
        Style::Brilho,
        Style::Aneis,
        Style::Linhas,
        Style::Blocos,
        Style::CGigante,
        Style::Diagonais,
        Style::Solido,
    ];

    pub fn id(self) -> &'static str {
        match self {
            Style::Solido => "solido",
            Style::Grade => "grade",
            Style::Aneis => "aneis",
            Style::Linhas => "linhas",
            Style::Blocos => "blocos",
            Style::CGigante => "c-gigante",
            Style::Diagonais => "diagonais",
            Style::Brilho => "brilho",
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Style::Solido => "sólido",
            Style::Grade => "grade",
            Style::Aneis => "anéis",
            Style::Linhas => "relevo",
            Style::Blocos => "janelas",
            Style::CGigante => "c gigante",
            Style::Diagonais => "diagonais",
            Style::Brilho => "brilho",
        }
    }

    pub fn desc(self) -> &'static str {
        match self {
            Style::Solido => "só a cor de fundo e a marca, quieta no canto",
            Style::Grade => "pontos em grade, que ganham o acento perto do foco",
            Style::Aneis => "ondas concêntricas saindo do canto, como um sonar",
            Style::Linhas => "linhas de relevo que sobem do rodapé",
            Style::Blocos => "janelas em tiling, uma delas com o foco",
            Style::CGigante => "a marca em tamanho gigante, cortada pela tela",
            Style::Diagonais => "listras em diagonal que aparecem aos poucos",
            Style::Brilho => "um brilho suave do acento no canto, sem forma nenhuma",
        }
    }
}

impl FromStr for Style {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> Result<Self> {
        let s = s.to_ascii_lowercase();
        Style::ALL.into_iter().find(|st| st.id() == s).with_context(|| {
            let ids: Vec<&str> = Style::ALL.iter().map(|s| s.id()).collect();
            format!("estilo {s:?} não existe. Estilos: {}", ids.join(", "))
        })
    }
}

// ── cores e utilidades ─────────────────────────────────────────────────────

fn rgbf(c: Rgb) -> V3 {
    [f32::from(c.r), f32::from(c.g), f32::from(c.b)]
}

fn lerp(a: V3, b: V3, t: f32) -> V3 {
    let t = t.clamp(0.0, 1.0);
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t]
}

/// Pinta `color` por cima de `base` com opacidade `a`.
fn over(base: V3, color: V3, a: f32) -> V3 {
    lerp(base, color, a)
}

fn smooth(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Cobertura de um pixel dado a distância com sinal até a borda (negativa = dentro).
fn cov(d: f32) -> f32 {
    (0.5 - d).clamp(0.0, 1.0)
}

fn hash(x: u32, y: u32) -> f32 {
    let mut h = x.wrapping_mul(0x9E37_79B1) ^ y.wrapping_mul(0x85EB_CA77);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2C1B_3C6D);
    h ^= h >> 12;
    h = h.wrapping_mul(0x297A_2D39);
    h ^= h >> 15;
    (h & 0xFFFF) as f32 / 65535.0
}

/// Distância até a linha mais próxima de uma família periódica.
fn periodic(v: f32, pitch: f32) -> (f32, i32) {
    let k = (v / pitch).round();
    ((v - k * pitch).abs(), k as i32)
}

struct Pal {
    bg: V3,
    surface: V3,
    raised: V3,
    line: V3,
    mute: V3,
    accent: V3,
    accent_dim: V3,
    accent_soft: V3,
}

impl Pal {
    fn of(c: &Colors) -> Self {
        Self {
            bg: rgbf(c.bg),
            surface: rgbf(c.surface),
            raised: rgbf(c.raised),
            line: rgbf(c.line),
            mute: rgbf(c.mute),
            accent: rgbf(c.accent),
            accent_dim: rgbf(c.accent_dim),
            accent_soft: rgbf(c.accent_soft),
        }
    }
}

// ── a marca ────────────────────────────────────────────────────────────────

/// Os vértices (x, y, raio) do "c", os mesmos de `brand/build.py`.
const MARK_VERTS: [(f32, f32, f32); 8] = [
    (0.0, 0.0, 6.0),
    (64.0, 0.0, 6.0),
    (64.0, 20.0, 3.0),
    (20.0, 20.0, 2.0),
    (20.0, 44.0, 2.0),
    (64.0, 44.0, 3.0),
    (64.0, 64.0, 6.0),
    (0.0, 64.0, 6.0),
];

/// Campo de distância com sinal do corpo da marca, em unidades da marca (caixa de 64).
/// Calculado uma vez numa grade e amostrado com interpolação: os cantos continuam suaves em qualquer escala.
struct Mark {
    grid: Vec<f32>,
    n: usize,
}

static MARK: std::sync::OnceLock<Mark> = std::sync::OnceLock::new();

const MARK_PAD: f32 = 10.0;
const MARK_RES: f32 = 8.0; // amostras por unidade

impl Mark {
    fn build() -> Self {
        let poly = flatten_rounded(&MARK_VERTS);
        let n = ((64.0 + 2.0 * MARK_PAD) * MARK_RES) as usize;
        let mut grid = vec![0.0; n * n];
        for j in 0..n {
            for i in 0..n {
                let p = (i as f32 / MARK_RES - MARK_PAD, j as f32 / MARK_RES - MARK_PAD);
                grid[j * n + i] = poly_sdf(&poly, p);
            }
        }
        Mark { grid, n }
    }

    /// Distância (em unidades da marca) do ponto `(ux, uy)` ao corpo.
    fn dist(&self, ux: f32, uy: f32) -> f32 {
        let fx = ((ux + MARK_PAD) * MARK_RES).clamp(0.0, (self.n - 1) as f32 - 0.001);
        let fy = ((uy + MARK_PAD) * MARK_RES).clamp(0.0, (self.n - 1) as f32 - 0.001);
        let (x0, y0) = (fx as usize, fy as usize);
        let (tx, ty) = (fx - x0 as f32, fy - y0 as f32);
        let g = |x: usize, y: usize| self.grid[y * self.n + x];
        let top = g(x0, y0) * (1.0 - tx) + g(x0 + 1, y0) * tx;
        let bottom = g(x0, y0 + 1) * (1.0 - tx) + g(x0 + 1, y0 + 1) * tx;
        top * (1.0 - ty) + bottom * ty
    }
}

/// O cursor da marca: caixa (34, 20, 12, 24) com raio 2.
fn cursor_sdf(ux: f32, uy: f32) -> f32 {
    let (cx, cy, hx, hy, r) = (40.0, 32.0, 6.0, 12.0, 2.0);
    let qx = (ux - cx).abs() - hx + r;
    let qy = (uy - cy).abs() - hy + r;
    qx.max(qy).min(0.0) + qx.max(0.0).hypot(qy.max(0.0)) - r
}

/// Polígono com cantos arredondados vira uma lista de pontos (arcos em 10 passos).
fn flatten_rounded(verts: &[(f32, f32, f32)]) -> Vec<(f32, f32)> {
    let n = verts.len();
    let mut out = Vec::new();
    for i in 0..n {
        let (x, y, r) = verts[i];
        let (px, py, _) = verts[(i + n - 1) % n];
        let (nx, ny, _) = verts[(i + 1) % n];
        let unit = |dx: f32, dy: f32| {
            let l = dx.hypot(dy);
            (dx / l, dy / l)
        };
        let d1 = unit(px - x, py - y);
        let d2 = unit(nx - x, ny - y);
        // Todos os ângulos da marca são retos: o centro do arco fica a `r` das duas arestas.
        let c = (x + (d1.0 + d2.0) * r, y + (d1.1 + d2.1) * r);
        let a = (x + d1.0 * r, y + d1.1 * r);
        let b = (x + d2.0 * r, y + d2.1 * r);
        if r == 0.0 {
            out.push((x, y));
            continue;
        }
        let a0 = (a.1 - c.1).atan2(a.0 - c.0);
        let mut sweep = (b.1 - c.1).atan2(b.0 - c.0) - a0;
        while sweep > std::f32::consts::PI {
            sweep -= 2.0 * std::f32::consts::PI;
        }
        while sweep < -std::f32::consts::PI {
            sweep += 2.0 * std::f32::consts::PI;
        }
        const STEPS: usize = 10;
        for s in 0..=STEPS {
            let t = a0 + sweep * s as f32 / STEPS as f32;
            out.push((c.0 + r * t.cos(), c.1 + r * t.sin()));
        }
    }
    out
}

/// Distância com sinal (negativa dentro) até um polígono.
fn poly_sdf(poly: &[(f32, f32)], p: (f32, f32)) -> f32 {
    let n = poly.len();
    let mut best = f32::MAX;
    let mut inside = false;
    for i in 0..n {
        let a = poly[i];
        let b = poly[(i + 1) % n];
        let (ex, ey) = (b.0 - a.0, b.1 - a.1);
        let (wx, wy) = (p.0 - a.0, p.1 - a.1);
        let t = ((wx * ex + wy * ey) / (ex * ex + ey * ey)).clamp(0.0, 1.0);
        best = best.min((wx - ex * t).hypot(wy - ey * t));
        // Raio horizontal para a direita: paridade das arestas cruzadas.
        if (a.1 > p.1) != (b.1 > p.1) && p.0 < a.0 + (p.1 - a.1) / (b.1 - a.1) * ex {
            inside = !inside;
        }
    }
    if inside { -best } else { best }
}

struct MarkPaint {
    ox: f32,
    oy: f32,
    scale: f32,
}

impl MarkPaint {
    /// Pinta a marca em `px`: corpo (`body`, opacidade `ba`) e cursor (`cur`, opacidade `ca`).
    fn paint(&self, m: &Mark, x: f32, y: f32, px: V3, body: (V3, f32), cur: (V3, f32)) -> V3 {
        let (ux, uy) = ((x - self.ox) / self.scale, (y - self.oy) / self.scale);
        if !(-2.0..=66.0).contains(&ux) || !(-2.0..=66.0).contains(&uy) {
            return px;
        }
        let body_cov = cov(m.dist(ux, uy) * self.scale);
        let cur_cov = cov(cursor_sdf(ux, uy) * self.scale);
        over(over(px, body.0, body_cov * body.1), cur.0, cur_cov * cur.1)
    }
}

// ── cenas ──────────────────────────────────────────────────────────────────

type Shader = Box<dyn Fn(f32, f32) -> V3 + Sync>;

/// `lw`: reforço da espessura das linhas (1.0 no papel de parede de verdade; miniaturas usam mais para as linhas aparecerem).
fn scene(style: Style, c: &Colors, w: f32, h: f32, lw: f32) -> Shader {
    let p = Pal::of(c);
    let u = h / 1080.0;
    let line_w = move |v: f32| (v * u).max(0.6) * lw;
    let mark: &'static Mark = MARK.get_or_init(Mark::build);

    // Marca pequena e quieta num canto.
    let corner_mark = {
        let size = 44.0 * u;
        let m = mark;
        move |paint_at: (f32, f32), x: f32, y: f32, px: V3, line: V3, cur: V3| {
            let mp = MarkPaint { ox: paint_at.0, oy: paint_at.1, scale: size / 64.0 };
            mp.paint(m, x, y, px, (line, 1.0), (cur, 1.0))
        }
    };

    match style {
        Style::Solido => {
            let at = (w - 100.0 * u, h - 100.0 * u);
            Box::new(move |x, y| corner_mark(at, x, y, p.bg, p.line, p.accent_dim))
        }
        Style::Brilho => {
            let at = (56.0 * u, 56.0 * u);
            Box::new(move |x, y| {
                let g1 = {
                    let d = ((x - 0.88 * w).powi(2) + (y - 1.02 * h).powi(2)).sqrt() / (0.85 * h);
                    (1.0 - smooth(0.0, 1.0, d)).powi(2)
                };
                let g2 = {
                    let d = ((x - 0.02 * w).powi(2) + (y + 0.12 * h).powi(2)).sqrt() / (0.7 * h);
                    (1.0 - smooth(0.0, 1.0, d)).powi(2)
                };
                let mut px = lerp(p.bg, p.accent, 0.30 * g1);
                px = lerp(px, p.line, 0.45 * g2);
                corner_mark(at, x, y, px, p.mute, p.accent_dim)
            })
        }
        Style::Grade => {
            let pitch = 36.0 * u;
            let (offx, offy) = (((w % pitch) / 2.0).max(pitch / 2.0), ((h % pitch) / 2.0).max(pitch / 2.0));
            let focus = (0.74 * w, 0.58 * h);
            let reach = 0.42 * h;
            let at = (w - 100.0 * u, h - 100.0 * u);
            Box::new(move |x, y| {
                let (cx, cy) =
                    (((x - offx) / pitch).round() * pitch + offx, ((y - offy) / pitch).round() * pitch + offy);
                let near = 1.0 - smooth(0.0, reach, (cx - focus.0).hypot(cy - focus.1));
                let glow = 1.0 - smooth(0.0, reach * 1.5, (x - focus.0).hypot(y - focus.1));
                let base = lerp(p.bg, p.accent_soft, 0.55 * glow);
                let radius = (1.0 + 2.1 * near) * u * lw.min(1.6);
                let col = lerp(p.line, p.accent, near.powf(1.4));
                let px = over(base, col, cov((x - cx).hypot(y - cy) - radius.max(0.9)));
                corner_mark(at, x, y, px, p.line, p.accent_dim)
            })
        }
        Style::Aneis => {
            let (cx, cy) = (0.80 * w, 1.08 * h);
            let gap = 48.0 * u;
            let at = (56.0 * u, 56.0 * u);
            Box::new(move |x, y| {
                let d = (x - cx).hypot(y - cy);
                let (off, k) = periodic(d, gap);
                let fade = 1.0 - smooth(0.2 * h, 1.45 * h, d);
                let col = if k % 6 == 0 { p.accent_dim } else { p.line };
                let a = cov(off - line_w(1.1) / 2.0) * (0.25 + 0.75 * fade) * if k >= 1 { 1.0 } else { 0.0 };
                let px = lerp(p.bg, p.accent_soft, 0.35 * (1.0 - smooth(0.0, 0.5 * h, d)));
                corner_mark(at, x, y, over(px, col, a), p.line, p.accent_dim)
            })
        }
        Style::Linhas => {
            let pitch = 13.0 * u;
            let at = (w - 100.0 * u, 56.0 * u);
            Box::new(move |x, y| {
                let t = y / h;
                let wave = 38.0
                    * u
                    * ((x / (190.0 * u)).sin() + 0.5 * (x / (83.0 * u) + 1.7).sin() + 0.25 * (x / (410.0 * u)).cos());
                let (off, _) = periodic(y + wave * (0.25 + 0.75 * t), pitch);
                let visible = smooth(0.12, 0.75, t);
                let glow = (-((t - 0.8) / 0.16).powi(2)).exp();
                let col = lerp(p.line, p.accent, glow);
                let a = cov(off - line_w(1.0) / 2.0) * visible;
                corner_mark(at, x, y, over(p.bg, col, a), p.line, p.accent_dim)
            })
        }
        Style::Blocos => {
            // Tiling "dwindle": cada nível entrega 58% da área a uma janela e divide o resto.
            let m = 0.1 * h;
            let mut rects: Vec<(f32, f32, f32, f32)> = Vec::new();
            let (mut rx, mut ry, mut rw, mut rh) = (m * 1.6, m, w - m * 3.2, h - 2.0 * m);
            for i in 0..6 {
                if i % 2 == 0 {
                    let first = rw * 0.58;
                    rects.push((rx, ry, first, rh));
                    rx += first;
                    rw -= first;
                } else {
                    let first = rh * 0.58;
                    rects.push((rx, ry, rw, first));
                    ry += first;
                    rh -= first;
                }
            }
            rects.push((rx, ry, rw, rh));
            let gap = 7.0 * u;
            let radius = 10.0 * u;
            let focused = 2;
            let big = rects[0];
            let mark_size = (big.3 * 0.32).min(big.2 * 0.5);
            let big_mark = MarkPaint {
                ox: big.0 + big.2 / 2.0 - mark_size / 2.0,
                oy: big.1 + big.3 / 2.0 - mark_size / 2.0,
                scale: mark_size / 64.0,
            };
            let mk = mark;
            Box::new(move |x, y| {
                let mut px = p.bg;
                for (i, &(rx, ry, rw, rh)) in rects.iter().enumerate() {
                    let (hx, hy) = (rw / 2.0 - gap, rh / 2.0 - gap);
                    let (qx, qy) = ((x - rx - rw / 2.0).abs() - hx + radius, (y - ry - rh / 2.0).abs() - hy + radius);
                    let d = qx.max(qy).min(0.0) + qx.max(0.0).hypot(qy.max(0.0)) - radius;
                    if d > 1.0 {
                        continue;
                    }
                    let (fill, edge, ew) =
                        if i == focused { (p.accent_soft, p.accent, 2.0) } else { (p.surface, p.line, 1.0) };
                    px = over(px, fill, cov(d));
                    px = over(px, edge, cov(d.abs() - line_w(ew) / 2.0));
                }
                big_mark.paint(mk, x, y, px, (p.raised, 1.0), (p.accent_dim, 1.0))
            })
        }
        Style::CGigante => {
            // Vaza por cima e por baixo, mas a abertura do "c" (com o cursor) fica toda à vista.
            let size = 1.35 * h;
            let paint = MarkPaint { ox: w - 0.97 * size, oy: (h - size) / 2.0, scale: size / 64.0 };
            let mk = mark;
            Box::new(move |x, y| {
                let (ux, uy) = ((x - paint.ox) / paint.scale, (y - paint.oy) / paint.scale);
                let d = mk.dist(ux, uy) * paint.scale;
                let mut px = over(p.bg, p.surface, cov(d));
                px = over(px, p.line, cov(d.abs() - line_w(1.5) / 2.0));
                over(px, p.accent, cov(cursor_sdf(ux, uy) * paint.scale) * 0.92)
            })
        }
        Style::Diagonais => {
            let pitch = 26.0 * u * std::f32::consts::SQRT_2;
            let at = (56.0 * u, 56.0 * u);
            Box::new(move |x, y| {
                let s = (x + y) / std::f32::consts::SQRT_2;
                let (off, k) = periodic(s, pitch);
                let t = 0.5 * (x / w + y / h);
                let visible = smooth(0.02, 0.85, t);
                let col = if k % 9 == 0 { p.accent } else { p.line };
                let strength = if k % 9 == 0 { 0.85 } else { 1.0 };
                let a = cov(off - line_w(1.0) / 2.0) * visible * strength;
                corner_mark(at, x, y, over(p.bg, col, a), p.line, p.accent_dim)
            })
        }
    }
}

/// Renderiza em RGB8 (`w * h * 3` bytes), em paralelo por faixas de linhas.
pub fn render(style: Style, c: &Colors, w: u32, h: u32) -> Vec<u8> {
    render_with(style, c, w, h, 1.0)
}

fn render_with(style: Style, c: &Colors, w: u32, h: u32, lw: f32) -> Vec<u8> {
    let shader = scene(style, c, w as f32, h as f32, lw);
    let mut out = vec![0u8; (w * h * 3) as usize];
    let threads = std::thread::available_parallelism().map_or(1, usize::from).min(16);
    let rows_per = (h as usize).div_ceil(threads).max(1);
    std::thread::scope(|s| {
        for (band, chunk) in out.chunks_mut(rows_per * w as usize * 3).enumerate() {
            let shader = &shader;
            s.spawn(move || {
                for (r, row) in chunk.chunks_mut(w as usize * 3).enumerate() {
                    let y = band * rows_per + r;
                    for (xi, px) in row.chunks_mut(3).enumerate() {
                        let v = shader(xi as f32 + 0.5, y as f32 + 0.5);
                        let n = hash(xi as u32, y as u32) - 0.5;
                        for k in 0..3 {
                            px[k] = (v[k] + n).round().clamp(0.0, 255.0) as u8;
                        }
                    }
                }
            });
        }
    });
    out
}

/// Miniatura para o terminal: `cols` x `rows` células, cada uma com dois pixels (meio-bloco `▀`).
/// Renderiza em 4x e reduz, com as linhas mais grossas, para o desenho aparecer em tamanho pequeno.
pub fn thumbnail(style: Style, c: &Colors, cols: u32, rows: u32) -> Vec<Rgb> {
    const SS: u32 = 4;
    let (w, h) = (cols * SS, rows * 2 * SS);
    let big = render_with(style, c, w, h, 3.0);
    let mut out = Vec::with_capacity((cols * rows * 2) as usize);
    for y in 0..rows * 2 {
        for x in 0..cols {
            let mut acc = [0u32; 3];
            for sy in 0..SS {
                for sx in 0..SS {
                    let i = (((y * SS + sy) * w + x * SS + sx) * 3) as usize;
                    for k in 0..3 {
                        acc[k] += u32::from(big[i + k]);
                    }
                }
            }
            let n = SS * SS;
            out.push(Rgb { r: (acc[0] / n) as u8, g: (acc[1] / n) as u8, b: (acc[2] / n) as u8 });
        }
    }
    out
}

pub fn encode_png(w: u32, h: u32, rgb: &[u8]) -> Result<Vec<u8>> {
    if rgb.len() != (w * h * 3) as usize {
        bail!("buffer de {} bytes não é uma imagem {w}x{h} RGB", rgb.len());
    }
    let mut out = Vec::new();
    {
        let mut enc = png::Encoder::new(&mut out, w, h);
        enc.set_color(png::ColorType::Rgb);
        enc.set_depth(png::BitDepth::Eight);
        // Rápido: o arquivo é gerado a cada troca de tema e vive em disco local.
        enc.set_compression(png::Compression::Fast);
        let mut writer = enc.write_header()?;
        writer.write_image_data(rgb)?;
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::Theme;
    use crate::tokens::{Mode, MotionLevel, Tokens};

    fn colors(mode: Mode, accent: &str) -> Colors {
        Theme::resolve(&Tokens::builtin(), mode, accent, MotionLevel::Full).unwrap().c
    }

    fn px(buf: &[u8], w: u32, x: u32, y: u32) -> [u8; 3] {
        let i = ((y * w + x) * 3) as usize;
        [buf[i], buf[i + 1], buf[i + 2]]
    }

    #[test]
    fn style_ids_roundtrip_and_are_unique() {
        for s in Style::ALL {
            assert_eq!(s.id().parse::<Style>().unwrap(), s);
        }
        let mut ids: Vec<_> = Style::ALL.iter().map(|s| s.id()).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), Style::ALL.len());
        assert!("banana".parse::<Style>().unwrap_err().to_string().contains("grade"));
    }

    #[test]
    fn every_style_renders_something_that_is_not_flat_in_every_theme() {
        for mode in [Mode::Dark, Mode::Light] {
            let c = colors(mode, "ember");
            for s in Style::ALL {
                let buf = render(s, &c, 320, 180);
                assert_eq!(buf.len(), 320 * 180 * 3);
                let first = &buf[..3];
                assert!(buf.chunks(3).any(|p| p != first), "{} em {mode} ficou chapado", s.id());
            }
        }
    }

    #[test]
    fn solid_is_background_with_the_mark_in_the_corner() {
        let c = colors(Mode::Dark, "azure");
        let buf = render(Style::Solido, &c, 640, 360);
        let bg = [c.bg.r, c.bg.g, c.bg.b];
        for (x, y) in [(10, 10), (320, 180), (10, 350)] {
            let p = px(&buf, 640, x, y);
            assert!(p.iter().zip(bg).all(|(a, b)| a.abs_diff(b) <= 1), "({x},{y}) = {p:?}, queria {bg:?}");
        }
        // o cursor da marca fica em (64 - ...) do canto: há pixel que não é fundo ali
        assert!(
            (220..640).step_by(2).any(|x| (200..360).step_by(2).any(|y| px(&buf, 640, x, y) != [bg[0], bg[1], bg[2]]))
        );
    }

    #[test]
    fn accent_shows_up_in_the_render_and_follows_the_theme() {
        let ember = render(Style::CGigante, &colors(Mode::Dark, "ember"), 320, 180);
        let azure = render(Style::CGigante, &colors(Mode::Dark, "azure"), 320, 180);
        assert_ne!(ember, azure);
        let light = render(Style::CGigante, &colors(Mode::Light, "ember"), 320, 180);
        assert_ne!(ember, light);
    }

    #[test]
    fn scale_is_relative_to_height() {
        // A mesma cena em duas resoluções tem que ter a mesma cara (compara com a menor, ampliada).
        let c = colors(Mode::Dark, "ember");
        let small = render(Style::CGigante, &c, 320, 180);
        let big = render(Style::CGigante, &c, 640, 360);
        let mut worst = 0u32;
        for y in 0..180 {
            for x in 0..320 {
                let a = px(&small, 320, x, y);
                let b = px(&big, 640, x * 2 + 1, y * 2 + 1);
                worst = worst.max(a.iter().zip(b).map(|(a, b)| u32::from(a.abs_diff(b))).max().unwrap());
            }
        }
        // Só as bordas antialiasadas podem divergir; nada de uma forma em outro lugar.
        let bad = (0..180).flat_map(|y| (0..320).map(move |x| (x, y))).filter(|&(x, y)| {
            let a = px(&small, 320, x, y);
            let b = px(&big, 640, x * 2 + 1, y * 2 + 1);
            a.iter().zip(b).any(|(a, b)| a.abs_diff(b) > 90)
        });
        assert!(bad.count() < 320 * 180 / 50, "as cenas divergem entre resoluções (pior diferença {worst})");
    }

    #[test]
    fn mark_sdf_matches_the_shape() {
        let m = Mark::build();
        assert!(m.dist(5.0, 32.0) < -3.0, "dentro da coluna do c");
        assert!(m.dist(40.0, 32.0) > 3.0, "dentro da abertura do c");
        assert!(m.dist(40.0, 10.0) < -3.0, "dentro do braço de cima");
        assert!(m.dist(-8.0, 32.0) > 6.0, "fora");
        // canto externo arredondado: o ponto da quina do quadrado está fora do corpo
        assert!(m.dist(0.3, 0.3) > 0.0, "quina arredondada");
    }

    #[test]
    fn thumbnail_has_two_pixels_per_cell() {
        let t = thumbnail(Style::Grade, &colors(Mode::Dark, "mint"), 40, 12);
        assert_eq!(t.len(), 40 * 12 * 2);
        assert!(t.windows(2).any(|w| w[0] != w[1]), "a miniatura não pode ser chapada");
    }

    #[test]
    fn png_roundtrips_through_the_decoder() {
        let c = colors(Mode::Dark, "rose");
        let rgb = render(Style::Aneis, &c, 64, 36);
        let bytes = encode_png(64, 36, &rgb).unwrap();
        let mut dec = png::Decoder::new(std::io::Cursor::new(bytes)).read_info().unwrap();
        let mut back = vec![0; dec.output_buffer_size().unwrap()];
        let info = dec.next_frame(&mut back).unwrap();
        assert_eq!((info.width, info.height), (64, 36));
        assert_eq!(&back[..rgb.len()], &rgb[..]);
        assert!(encode_png(10, 10, &rgb).is_err());
    }
}
