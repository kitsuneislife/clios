//! Os brinquedos do terminal (cbonsai, pipes, lavat...) e a ponte entre eles e o tema.
//!
//! Quase nenhum deles aceita cor em RGB: falam as oito cores ANSI. Aqui a gente escolhe, entre as seis
//! cores com matiz, a mais perto do acento do usuário, e entrega no formato que o app entende.
//! Os que aceitam mais (o lavat em modo gradiente fala hex; o csakura tem paletas com nome) recebem mais.
//!
//! Os marcadores que um comando do catálogo pode usar:
//!   `{color}`    nome ANSI mais perto do acento (red, green, yellow, blue, magenta, cyan, white)
//!   `{n}`        o número dele (1 a 7)
//!   `{hex}`      o acento em RRGGBB
//!   `{hexdim}`   o acento recuado em RRGGBB (o segundo tom de um gradiente)
//!   `{palette}`  a paleta de flor do csakura que combina com o acento

use clios_core::{Rgb, Theme};

use crate::catalog::TuiDef;

/// Nome (como `cmatrix` e `lavat` pedem) e número ANSI (como `pipes.sh` e `tty-clock` pedem).
const HUES: [(&str, u8, f64); 6] = [
    ("red", 1, 0.0),
    ("yellow", 3, 60.0),
    ("green", 2, 120.0),
    ("cyan", 6, 180.0),
    ("blue", 4, 240.0),
    ("magenta", 5, 300.0),
];

/// Matiz em graus (0..360) e saturação (0..1).
fn hue_sat(c: Rgb) -> (f64, f64) {
    let (r, g, b) = (f64::from(c.r) / 255.0, f64::from(c.g) / 255.0, f64::from(c.b) / 255.0);
    let (max, min) = (r.max(g).max(b), r.min(g).min(b));
    let d = max - min;
    if d == 0.0 {
        return (0.0, 0.0);
    }
    let h = if max == r {
        ((g - b) / d).rem_euclid(6.0)
    } else if max == g {
        (b - r) / d + 2.0
    } else {
        (r - g) / d + 4.0
    };
    (h * 60.0, d / max)
}

/// A cor ANSI mais perto do acento. Acento sem cor (cinza, branco) vira `white`.
pub fn nearest_ansi(accent: Rgb) -> (&'static str, u8) {
    let (h, s) = hue_sat(accent);
    if s < 0.18 {
        return ("white", 7);
    }
    let dist = |a: f64| {
        let d = (a - h).abs();
        d.min(360.0 - d)
    };
    let (name, n, _) = HUES.iter().min_by(|a, b| dist(a.2).total_cmp(&dist(b.2))).copied().expect("HUES não é vazio");
    (name, n)
}

/// A paleta do csakura que combina com o acento. Cada acento do CLIOS tem a sua; um hex qualquer fica na padrão.
pub fn sakura_palette(accent_name: &str) -> &'static str {
    match accent_name {
        "ember" => "coral",
        "azure" => "sky",
        "violet" => "violet",
        "rose" => "rose",
        "mint" => "mint",
        "signal" => "gold",
        "mono" => "white",
        _ => "sakura",
    }
}

/// Troca os marcadores (veja o topo do arquivo) nos argumentos.
pub fn expand(argv: &[String], theme: &Theme) -> Vec<String> {
    let (name, n) = nearest_ansi(theme.c.accent);
    let n = n.to_string();
    let palette = sakura_palette(&theme.accent_name);
    argv.iter()
        .map(|a| {
            a.replace("{color}", name)
                .replace("{n}", &n)
                .replace("{hexdim}", &theme.c.accent_dim.bare())
                .replace("{hex}", &theme.c.accent.bare())
                .replace("{palette}", palette)
        })
        .collect()
}

/// As cenas disponíveis para a proteção de tela: os brinquedos instalados que sabem rodar sozinhos.
pub fn installed_scenes(tuis: &[TuiDef]) -> Vec<&TuiDef> {
    tuis.iter().filter(|t| !t.saver.is_empty() && t.installed()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog;
    use clios_core::{Mode, MotionLevel, Theme, Tokens};

    fn rgb(h: &str) -> Rgb {
        Rgb::parse(h).unwrap()
    }

    #[test]
    fn nearest_ansi_follows_the_hue() {
        assert_eq!(nearest_ansi(rgb("#FF2020")), ("red", 1));
        assert_eq!(
            nearest_ansi(rgb("#FF9A00")),
            ("yellow", 3),
            "laranja fica entre vermelho e amarelo, perto do amarelo"
        );
        assert_eq!(nearest_ansi(rgb("#3DFF7A")), ("green", 2));
        assert_eq!(nearest_ansi(rgb("#00E5E5")), ("cyan", 6));
        assert_eq!(nearest_ansi(rgb("#3A6BFF")), ("blue", 4));
        assert_eq!(nearest_ansi(rgb("#E040FF")), ("magenta", 5));
        assert_eq!(
            nearest_ansi(rgb("#FF20A0")),
            ("magenta", 5),
            "rosa-choque está mais perto do magenta que do vermelho"
        );
    }

    #[test]
    fn grey_and_white_accents_become_white() {
        assert_eq!(nearest_ansi(rgb("#FFFFFF")), ("white", 7));
        assert_eq!(nearest_ansi(rgb("#8C8C8C")), ("white", 7));
    }

    #[test]
    fn hue_wraps_around_red() {
        // 350° está a 10° do vermelho, não a 290° do magenta
        assert_eq!(nearest_ansi(rgb("#FF0020")).0, "red");
    }

    fn theme(accent: &str) -> Theme {
        Theme::resolve(&Tokens::builtin(), Mode::Dark, accent, MotionLevel::Full).unwrap()
    }

    #[test]
    fn expand_replaces_every_placeholder_and_leaves_the_rest() {
        let argv: Vec<String> =
            ["x", "-c", "{color}", "-C", "{n}", "-g", "{hex}", "{hexdim}", "-p", "{palette}", "--keep"]
                .map(String::from)
                .to_vec();
        let t = theme("mint");
        let out = expand(&argv, &t);
        assert_eq!(&out[..5], ["x", "-c", "cyan", "-C", "6"], "o mint tem matiz de 160°, mais perto do ciano");
        assert_eq!(out[6], t.c.accent.bare(), "o acento exato, para os que falam hex");
        assert_eq!(out[7], t.c.accent_dim.bare());
        assert_eq!((out[9].as_str(), out[10].as_str()), ("mint", "--keep"));
    }

    #[test]
    fn every_accent_has_a_sakura_palette_that_csakura_knows() {
        // as 15 paletas do csakura 2.1
        let known = [
            "sakura", "rose", "blush", "magenta", "peach", "coral", "sunset", "gold", "lavender", "violet", "sky",
            "mint", "matcha", "white", "ink",
        ];
        for name in Tokens::builtin().accent.keys() {
            assert!(known.contains(&sakura_palette(name)), "{name}");
        }
        assert_eq!(sakura_palette("custom"), "sakura");
    }

    #[test]
    fn every_accent_of_the_design_maps_to_something() {
        let t = Tokens::builtin();
        for name in t.accent.keys() {
            let th = Theme::resolve(&t, Mode::Dark, name, MotionLevel::Full).unwrap();
            let (c, n) = nearest_ansi(th.c.accent);
            assert!((1..=7).contains(&n), "{name}: {c} {n}");
        }
    }

    #[test]
    fn the_catalog_toys_use_only_known_placeholders() {
        for t in catalog::builtin().tui.iter().filter(|t| !t.saver.is_empty()) {
            for a in t.cmd.iter().chain(&t.saver) {
                let stripped = ["{color}", "{n}", "{hex}", "{hexdim}", "{palette}"]
                    .iter()
                    .fold(a.clone(), |s, p| s.replace(p, ""));
                assert!(!stripped.contains('{') && !stripped.contains('}'), "{}: placeholder estranho em {a:?}", t.id);
            }
            assert_eq!(t.category, "diversao", "{}: só brinquedos têm saver", t.id);
        }
    }

    #[test]
    fn there_are_enough_toys_to_rotate() {
        let toys = catalog::builtin().tui.iter().filter(|t| !t.saver.is_empty()).count();
        assert!(toys >= 6, "{toys} cenas");
    }
}
