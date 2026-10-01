//! Resolve tokens + (modo, acento, movimento) no tema concreto que os templates veem.

use std::collections::BTreeMap;

use anyhow::{Result, bail};
use serde::Serialize;

use crate::color::Rgb;
use crate::tokens::{Durations, Font, Mode, MotionLevel, Tokens, Ui};

/// Contraste mínimo do acento contra o fundo (texto normal, WCAG AA).
pub const MIN_ACCENT_CONTRAST: f64 = 4.5;

#[derive(Debug, Clone, Serialize)]
pub struct Theme {
    pub mode: Mode,
    pub accent_name: String,
    pub font: Font,
    pub ui: Ui,
    pub motion: Motion,
    pub c: Colors,
    /// Todos os acentos na variante do modo atual, para seletores.
    pub accents: Vec<Swatch>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Swatch {
    pub name: String,
    pub color: Rgb,
}

#[derive(Debug, Clone, Serialize)]
pub struct Motion {
    pub level: MotionLevel,
    /// `false` desliga as animações do compositor por completo.
    pub enabled: bool,
    /// `false` troca deslizes e escalas por fades.
    pub spatial: bool,
    pub duration: Durations,
    pub curve: BTreeMap<String, Curve>,
    pub spring: BTreeMap<String, Spring>,
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct Curve {
    pub x1: f64,
    pub y1: f64,
    pub x2: f64,
    pub y2: f64,
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct Spring {
    pub mass: f64,
    pub stiffness: f64,
    /// Coeficiente de amortecimento: `2 * zeta * sqrt(k * m)`.
    pub damping: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct Colors {
    pub bg: Rgb,
    pub surface: Rgb,
    pub raised: Rgb,
    pub line: Rgb,
    pub mute: Rgb,
    pub dim: Rgb,
    pub fg: Rgb,

    pub accent: Rgb,
    /// Acento recuado, para estados inativos.
    pub accent_dim: Rgb,
    /// Tinta do acento sobre o fundo, para seleção.
    pub accent_soft: Rgb,
    /// Cor de texto legível sobre `accent`.
    pub on_accent: Rgb,

    pub black: Rgb,
    pub red: Rgb,
    pub green: Rgb,
    pub yellow: Rgb,
    pub blue: Rgb,
    pub magenta: Rgb,
    pub cyan: Rgb,
    pub white: Rgb,
    pub bright_black: Rgb,
    pub bright_red: Rgb,
    pub bright_green: Rgb,
    pub bright_yellow: Rgb,
    pub bright_blue: Rgb,
    pub bright_magenta: Rgb,
    pub bright_cyan: Rgb,
    pub bright_white: Rgb,

    /// As 16 cores ANSI em ordem (0..=15).
    pub ansi: Vec<Rgb>,
}

impl Theme {
    pub fn resolve(tokens: &Tokens, mode: Mode, accent: &str, level: MotionLevel) -> Result<Theme> {
        let p = tokens.palette(mode);
        let accent_color = resolve_accent(tokens, mode, accent)?;
        // `resolve_accent` já recusou o que não é nome nem hex, então o que sobra é custom.
        let accent_name = if tokens.accent.contains_key(accent) { accent.to_string() } else { "custom".to_string() };

        // Brilhantes: a cor normal puxada 20% para o tom de texto do modo.
        let bright = |c: Rgb| c.mix(p.fg, 0.20);
        let a = &p.ansi;
        let bright_white = match mode {
            Mode::Dark => p.fg,
            Mode::Light => p.bg,
        };

        let c = Colors {
            bg: p.bg,
            surface: p.surface,
            raised: p.raised,
            line: p.line,
            mute: p.mute,
            dim: p.dim,
            fg: p.fg,
            accent: accent_color,
            accent_dim: p.bg.blend(accent_color, 0.55),
            accent_soft: p.bg.blend(accent_color, 0.18),
            on_accent: accent_color.best_of(p.bg, p.fg),
            black: a.black,
            red: a.red,
            green: a.green,
            yellow: a.yellow,
            blue: a.blue,
            magenta: a.magenta,
            cyan: a.cyan,
            white: a.white,
            bright_black: p.mute,
            bright_red: bright(a.red),
            bright_green: bright(a.green),
            bright_yellow: bright(a.yellow),
            bright_blue: bright(a.blue),
            bright_magenta: bright(a.magenta),
            bright_cyan: bright(a.cyan),
            bright_white,
            ansi: vec![
                a.black,
                a.red,
                a.green,
                a.yellow,
                a.blue,
                a.magenta,
                a.cyan,
                a.white,
                p.mute,
                bright(a.red),
                bright(a.green),
                bright(a.yellow),
                bright(a.blue),
                bright(a.magenta),
                bright(a.cyan),
                bright_white,
            ],
        };

        let mut accents: Vec<Swatch> = tokens
            .accent
            .iter()
            .map(|(name, def)| Swatch {
                name: name.clone(),
                color: match mode {
                    Mode::Dark => def.dark,
                    Mode::Light => def.light,
                },
            })
            .collect();
        // `mono` por último: é a opção neutra, não uma cor.
        accents.sort_by_key(|s| s.name == "mono");

        Ok(Theme {
            mode,
            accent_name,
            font: tokens.font.clone(),
            ui: tokens.ui.clone(),
            motion: resolve_motion(tokens, level),
            c,
            accents,
        })
    }
}

fn resolve_accent(tokens: &Tokens, mode: Mode, accent: &str) -> Result<Rgb> {
    let p = tokens.palette(mode);
    if let Some(def) = tokens.accent.get(accent) {
        return Ok(match mode {
            Mode::Dark => def.dark,
            Mode::Light => def.light,
        });
    }
    match Rgb::parse(accent) {
        Ok(custom) => Ok(ensure_contrast(custom, p.bg, p.fg, MIN_ACCENT_CONTRAST)),
        Err(_) => {
            let names: Vec<&str> = tokens.accent.keys().map(String::as_str).collect();
            bail!("acento {accent:?} não existe. Use {} ou um hex (#RRGGBB).", names.join(", "))
        }
    }
}

/// Empurra `color` na direção de `toward` até atingir `min` de contraste contra `bg`.
/// Mantém o matiz e muda só a luminosidade, então um acento custom continua parecendo ele mesmo.
pub fn ensure_contrast(color: Rgb, bg: Rgb, toward: Rgb, min: f64) -> Rgb {
    if color.contrast(bg) >= min {
        return color;
    }
    for step in 1..=50 {
        let candidate = color.mix(toward, f64::from(step) / 50.0);
        if candidate.contrast(bg) >= min {
            return candidate;
        }
    }
    toward
}

fn resolve_motion(tokens: &Tokens, level: MotionLevel) -> Motion {
    let m = &tokens.motion;
    let scale = match level {
        MotionLevel::Full => 1.0,
        MotionLevel::Reduced => 0.5,
        MotionLevel::Off => 0.0,
    };
    let ms = |v: u32| (f64::from(v) * scale).round() as u32;
    Motion {
        level,
        enabled: level != MotionLevel::Off,
        spatial: level == MotionLevel::Full,
        duration: Durations {
            instant: ms(m.duration.instant),
            fast: ms(m.duration.fast),
            base: ms(m.duration.base),
            slow: ms(m.duration.slow),
        },
        curve: m
            .curve
            .iter()
            .map(|(k, v)| {
                let [x1, y1, x2, y2] = v.points;
                (k.clone(), Curve { x1, y1, x2, y2 })
            })
            .collect(),
        spring: m
            .spring
            .iter()
            .map(|(k, v)| {
                let damping = 2.0 * v.damping_ratio * (v.stiffness * v.mass).sqrt();
                (k.clone(), Spring { mass: v.mass, stiffness: v.stiffness, damping })
            })
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn theme(mode: Mode, accent: &str) -> Theme {
        Theme::resolve(&Tokens::builtin(), mode, accent, MotionLevel::Full).unwrap()
    }

    #[test]
    fn text_colors_meet_wcag_aa_in_both_modes() {
        for mode in [Mode::Dark, Mode::Light] {
            let t = theme(mode, "ember");
            assert!(t.c.fg.contrast(t.c.bg) >= 7.0, "{mode}: fg");
            assert!(t.c.dim.contrast(t.c.bg) >= 7.0, "{mode}: dim");
            assert!(t.c.mute.contrast(t.c.bg) >= 3.0, "{mode}: mute (apenas dica, não leitura)");
            assert!(t.c.fg.contrast(t.c.surface) >= 7.0, "{mode}: fg em surface");
            assert!(t.c.fg.contrast(t.c.raised) >= 7.0, "{mode}: fg em raised");
        }
    }

    #[test]
    fn every_accent_is_readable_on_bg_in_both_modes() {
        let tokens = Tokens::builtin();
        for mode in [Mode::Dark, Mode::Light] {
            for name in tokens.accent.keys() {
                let t = theme(mode, name);
                let ratio = t.c.accent.contrast(t.c.bg);
                assert!(ratio >= MIN_ACCENT_CONTRAST, "acento {name} em {mode}: {ratio:.2} < {MIN_ACCENT_CONTRAST}");
                let on = t.c.on_accent.contrast(t.c.accent);
                assert!(on >= MIN_ACCENT_CONTRAST, "texto sobre {name} em {mode}: {on:.2}");
            }
        }
    }

    #[test]
    fn selection_tint_is_visible_but_keeps_text_readable() {
        let tokens = Tokens::builtin();
        for mode in [Mode::Dark, Mode::Light] {
            for name in tokens.accent.keys() {
                let t = theme(mode, name);
                let seen = t.c.accent_soft.contrast(t.c.bg);
                assert!(seen >= 1.08, "{name} em {mode}: tinta invisível ({seen:.3})");
                assert!(t.c.fg.contrast(t.c.accent_soft) >= 7.0, "{name} em {mode}: texto sobre a tinta");
                assert!(t.c.accent_dim.contrast(t.c.bg) >= 2.0, "{name} em {mode}: acento recuado some");
            }
        }
    }

    #[test]
    fn ansi_hues_are_readable_on_bg() {
        for mode in [Mode::Dark, Mode::Light] {
            let t = theme(mode, "ember");
            for (name, c) in [
                ("red", t.c.red),
                ("green", t.c.green),
                ("yellow", t.c.yellow),
                ("blue", t.c.blue),
                ("magenta", t.c.magenta),
                ("cyan", t.c.cyan),
                ("bright_red", t.c.bright_red),
                ("bright_green", t.c.bright_green),
                ("bright_yellow", t.c.bright_yellow),
                ("bright_blue", t.c.bright_blue),
                ("bright_magenta", t.c.bright_magenta),
                ("bright_cyan", t.c.bright_cyan),
            ] {
                assert!(c.contrast(t.c.bg) >= 4.5, "{name} em {mode}: {:.2}", c.contrast(t.c.bg));
            }
        }
    }

    #[test]
    fn ansi_has_sixteen_entries_in_standard_order() {
        let t = theme(Mode::Dark, "ember");
        assert_eq!(t.c.ansi.len(), 16);
        assert_eq!(t.c.ansi[1], t.c.red);
        assert_eq!(t.c.ansi[8], t.c.mute);
        assert_eq!(t.c.ansi[15], t.c.fg);
    }

    #[test]
    fn custom_accent_is_lifted_to_readable_contrast() {
        // Um verde-limão fica ilegível no fundo branco; tem que ser escurecido.
        let t = theme(Mode::Light, "#7CFF00");
        assert_eq!(t.accent_name, "custom");
        assert!(t.c.accent.contrast(t.c.bg) >= MIN_ACCENT_CONTRAST);
        // Já em fundo preto ele passa sem alteração.
        let d = theme(Mode::Dark, "#7CFF00");
        assert_eq!(d.c.accent, Rgb::parse("#7CFF00").unwrap());
    }

    #[test]
    fn unknown_accent_lists_the_options() {
        let err = Theme::resolve(&Tokens::builtin(), Mode::Dark, "banana", MotionLevel::Full).unwrap_err().to_string();
        assert!(err.contains("ember") && err.contains("azure"), "{err}");
    }

    #[test]
    fn critical_damping_matches_the_formula() {
        let t = theme(Mode::Dark, "ember");
        let s = t.motion.spring["settle"];
        // zeta = 1: damping = 2 * sqrt(k * m)
        assert!((s.damping - 2.0 * (s.stiffness * s.mass).sqrt()).abs() < 1e-9);
    }

    #[test]
    fn motion_levels_scale_durations() {
        let tokens = Tokens::builtin();
        let full = Theme::resolve(&tokens, Mode::Dark, "ember", MotionLevel::Full).unwrap();
        let reduced = Theme::resolve(&tokens, Mode::Dark, "ember", MotionLevel::Reduced).unwrap();
        let off = Theme::resolve(&tokens, Mode::Dark, "ember", MotionLevel::Off).unwrap();
        assert_eq!(full.motion.duration.base, 240);
        assert_eq!(reduced.motion.duration.base, 120);
        assert!(reduced.motion.enabled && !reduced.motion.spatial);
        assert_eq!(off.motion.duration.base, 0);
        assert!(!off.motion.enabled);
    }

    #[test]
    fn exits_are_never_slower_than_entries() {
        // Princípio 1 do manifesto de movimento: as durações crescem na ordem certa.
        let d = theme(Mode::Dark, "ember").motion.duration;
        assert!(d.instant < d.fast && d.fast < d.base && d.base < d.slow);
    }

    #[test]
    fn every_curve_stays_inside_the_unit_square_on_x() {
        // Um bezier cúbico com x fora de [0,1] deixa de ser uma função do tempo.
        let t = theme(Mode::Dark, "ember");
        for (name, c) in &t.motion.curve {
            assert!((0.0..=1.0).contains(&c.x1) && (0.0..=1.0).contains(&c.x2), "{name}");
        }
    }
}
