//! Movimento do hub: a mesma curva e as mesmas durações do resto do desktop.

use std::time::Duration;

use clios_core::{Rgb, Theme};

/// Bezier cúbico no estilo CSS: `cubic-bezier(x1, y1, x2, y2)`.
#[derive(Debug, Clone, Copy)]
pub struct Bezier {
    x1: f64,
    y1: f64,
    x2: f64,
    y2: f64,
}

impl Bezier {
    pub fn new(x1: f64, y1: f64, x2: f64, y2: f64) -> Self {
        Self { x1, y1, x2, y2 }
    }

    fn bx(&self, t: f64) -> f64 {
        let u = 1.0 - t;
        3.0 * u * u * t * self.x1 + 3.0 * u * t * t * self.x2 + t * t * t
    }

    fn by(&self, t: f64) -> f64 {
        let u = 1.0 - t;
        3.0 * u * u * t * self.y1 + 3.0 * u * t * t * self.y2 + t * t * t
    }

    fn dbx(&self, t: f64) -> f64 {
        let u = 1.0 - t;
        3.0 * u * u * self.x1 + 6.0 * u * t * (self.x2 - self.x1) + 3.0 * t * t * (1.0 - self.x2)
    }

    /// Valor da curva para o tempo normalizado `x` em [0, 1].
    pub fn ease(&self, x: f64) -> f64 {
        let x = x.clamp(0.0, 1.0);
        if x == 0.0 || x == 1.0 {
            return x;
        }
        // Newton-Raphson para achar t tal que bx(t) = x; bisseção se a derivada degenerar.
        let mut t = x;
        for _ in 0..8 {
            let err = self.bx(t) - x;
            if err.abs() < 1e-6 {
                return self.by(t);
            }
            let d = self.dbx(t);
            if d.abs() < 1e-6 {
                break;
            }
            t -= err / d;
        }
        let (mut lo, mut hi) = (0.0_f64, 1.0_f64);
        t = x;
        for _ in 0..40 {
            let v = self.bx(t);
            if (v - x).abs() < 1e-6 {
                break;
            }
            if v < x {
                lo = t
            } else {
                hi = t
            }
            t = (lo + hi) / 2.0;
        }
        self.by(t)
    }
}

/// Parâmetros de movimento do hub, vindos dos tokens.
#[derive(Debug, Clone, Copy)]
pub struct Motion {
    pub curve: Bezier,
    /// Troca de seleção.
    pub instant: u32,
    /// Entrada das linhas.
    pub fast: u32,
    /// Atraso entre uma linha e a próxima na entrada. 0 desliga o escalonamento.
    pub stagger: u32,
}

impl Motion {
    pub fn from_theme(theme: &Theme) -> Self {
        let c = theme.motion.curve.get("out").copied();
        let curve = c.map_or(Bezier::new(0.16, 1.0, 0.3, 1.0), |c| Bezier::new(c.x1, c.y1, c.x2, c.y2));
        let d = theme.motion.duration;
        Self {
            curve,
            instant: d.instant,
            fast: d.fast,
            // Só o movimento completo escalona; `reduced` mostra tudo junto, em fade curto.
            stagger: if theme.motion.spatial { 14 } else { 0 },
        }
    }

    /// Sem animação nenhuma.
    #[cfg(test)]
    pub fn none() -> Self {
        Self { curve: Bezier::new(0.0, 0.0, 1.0, 1.0), instant: 0, fast: 0, stagger: 0 }
    }

    /// Progresso eased (0 a 1) de algo que durou `elapsed` de um total `dur_ms`.
    pub fn progress(&self, elapsed: Duration, dur_ms: u32) -> f64 {
        if dur_ms == 0 {
            return 1.0;
        }
        self.curve.ease(elapsed.as_secs_f64() * 1000.0 / f64::from(dur_ms))
    }

    /// Tempo total até a entrada de `rows` linhas terminar.
    pub fn reveal_total(&self, rows: usize) -> Duration {
        let ms = u64::from(self.fast) + u64::from(self.stagger) * rows.saturating_sub(1) as u64;
        Duration::from_millis(ms)
    }
}

/// Cor `a` indo para `b` em `t` (alfa de composição, não mistura perceptual).
pub fn fade(a: Rgb, b: Rgb, t: f64) -> Rgb {
    a.blend(b, t)
}

#[cfg(test)]
mod tests {
    use super::*;
    use clios_core::{Mode, MotionLevel, Tokens};

    fn out() -> Bezier {
        Bezier::new(0.16, 1.0, 0.3, 1.0)
    }

    #[test]
    fn endpoints_are_fixed() {
        assert_eq!(out().ease(0.0), 0.0);
        assert_eq!(out().ease(1.0), 1.0);
        assert_eq!(out().ease(-5.0), 0.0);
        assert_eq!(out().ease(9.0), 1.0);
    }

    #[test]
    fn linear_curve_is_identity() {
        let l = Bezier::new(0.0, 0.0, 1.0, 1.0);
        for i in 0..=10 {
            let x = f64::from(i) / 10.0;
            assert!((l.ease(x) - x).abs() < 1e-4, "{x}");
        }
    }

    #[test]
    fn out_curve_decelerates() {
        // Uma curva "out" cobre a maior parte da distância no começo.
        assert!(out().ease(0.25) > 0.6, "{}", out().ease(0.25));
        assert!(out().ease(0.5) > 0.9);
    }

    #[test]
    fn curve_is_monotonic() {
        let c = out();
        let mut prev = 0.0;
        for i in 0..=200 {
            let v = c.ease(f64::from(i) / 200.0);
            assert!(v + 1e-9 >= prev, "recuou em {i}");
            prev = v;
        }
    }

    #[test]
    fn motion_follows_the_theme_levels() {
        let tokens = Tokens::builtin();
        let full = Motion::from_theme(&Theme::resolve(&tokens, Mode::Dark, "ember", MotionLevel::Full).unwrap());
        let reduced = Motion::from_theme(&Theme::resolve(&tokens, Mode::Dark, "ember", MotionLevel::Reduced).unwrap());
        let off = Motion::from_theme(&Theme::resolve(&tokens, Mode::Dark, "ember", MotionLevel::Off).unwrap());
        assert_eq!((full.instant, full.fast, full.stagger), (90, 160, 14));
        assert_eq!(reduced.stagger, 0);
        assert_eq!((off.instant, off.fast), (0, 0));
        assert_eq!(off.progress(Duration::ZERO, off.fast), 1.0, "sem movimento, tudo já chegou");
    }

    #[test]
    fn reveal_total_grows_with_rows() {
        let m = Motion { stagger: 14, fast: 160, ..Motion::none() };
        assert_eq!(m.reveal_total(1), Duration::from_millis(160));
        assert_eq!(m.reveal_total(11), Duration::from_millis(160 + 140));
    }

    #[test]
    fn fade_endpoints() {
        let (a, b) = (Rgb::new(0, 0, 0), Rgb::new(200, 100, 50));
        assert_eq!(fade(a, b, 0.0), a);
        assert_eq!(fade(a, b, 1.0), b);
    }
}
