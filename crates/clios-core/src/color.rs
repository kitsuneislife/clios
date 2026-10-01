//! Cor: parse, formatação, contraste WCAG e mistura perceptual (OKLab).

use std::fmt;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Rgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

#[derive(Debug)]
pub enum ColorError {
    Invalid(String),
}

impl fmt::Display for ColorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ColorError::Invalid(s) => write!(f, "cor inválida: {s:?} (esperado #RGB ou #RRGGBB)"),
        }
    }
}

impl std::error::Error for ColorError {}

impl Rgb {
    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    pub fn parse(input: &str) -> Result<Self, ColorError> {
        let s = input.trim().trim_start_matches('#');
        let bad = || ColorError::Invalid(input.to_string());
        let nibble = |c: char| c.to_digit(16).map(|d| d as u8).ok_or_else(bad);
        let chars: Vec<char> = s.chars().collect();
        match chars.len() {
            3 => {
                let [r, g, b] = [chars[0], chars[1], chars[2]].map(nibble);
                let (r, g, b) = (r?, g?, b?);
                Ok(Self::new(r * 17, g * 17, b * 17))
            }
            6 => {
                let byte = |i: usize| -> Result<u8, ColorError> { Ok(nibble(chars[i])? * 16 + nibble(chars[i + 1])?) };
                Ok(Self::new(byte(0)?, byte(2)?, byte(4)?))
            }
            _ => Err(bad()),
        }
    }

    /// `#RRGGBB`, maiúsculo.
    pub fn hex(self) -> String {
        format!("#{:02X}{:02X}{:02X}", self.r, self.g, self.b)
    }

    /// `RRGGBB`, maiúsculo, sem `#`.
    pub fn bare(self) -> String {
        format!("{:02X}{:02X}{:02X}", self.r, self.g, self.b)
    }

    /// Luminância relativa (WCAG 2.x), 0.0 a 1.0.
    pub fn luminance(self) -> f64 {
        let lin = |c: u8| {
            let c = f64::from(c) / 255.0;
            if c <= 0.03928 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
        };
        0.2126 * lin(self.r) + 0.7152 * lin(self.g) + 0.0722 * lin(self.b)
    }

    /// Razão de contraste WCAG, 1.0 a 21.0.
    pub fn contrast(self, other: Rgb) -> f64 {
        let (a, b) = (self.luminance(), other.luminance());
        let (hi, lo) = if a >= b { (a, b) } else { (b, a) };
        (hi + 0.05) / (lo + 0.05)
    }

    /// Dentre `a` e `b`, a cor com mais contraste contra `self`.
    pub fn best_of(self, a: Rgb, b: Rgb) -> Rgb {
        if self.contrast(a) >= self.contrast(b) { a } else { b }
    }

    /// Composição alfa em sRGB: `other` por cima de `self` com opacidade `t`.
    /// É o que o olho espera de uma "tinta" sobre o fundo (seleção, estado inativo).
    /// Para gradientes entre cores use `mix`, que é perceptual.
    pub fn blend(self, other: Rgb, t: f64) -> Rgb {
        let t = t.clamp(0.0, 1.0);
        let ch = |a: u8, b: u8| (f64::from(a) + (f64::from(b) - f64::from(a)) * t).round() as u8;
        Rgb::new(ch(self.r, other.r), ch(self.g, other.g), ch(self.b, other.b))
    }

    /// Mistura em OKLab. `t = 0` devolve `self`, `t = 1` devolve `other`.
    pub fn mix(self, other: Rgb, t: f64) -> Rgb {
        let t = t.clamp(0.0, 1.0);
        let (a, b) = (Oklab::from(self), Oklab::from(other));
        Oklab { l: a.l + (b.l - a.l) * t, a: a.a + (b.a - a.a) * t, b: a.b + (b.b - a.b) * t }.into()
    }
}

impl fmt::Display for Rgb {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.hex())
    }
}

impl Serialize for Rgb {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.hex())
    }
}

impl<'de> Deserialize<'de> for Rgb {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        Rgb::parse(&s).map_err(serde::de::Error::custom)
    }
}

#[derive(Clone, Copy)]
struct Oklab {
    l: f64,
    a: f64,
    b: f64,
}

fn to_linear(c: u8) -> f64 {
    let c = f64::from(c) / 255.0;
    if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
}

fn from_linear(c: f64) -> u8 {
    let c = c.clamp(0.0, 1.0);
    let v = if c <= 0.003_130_8 { c * 12.92 } else { 1.055 * c.powf(1.0 / 2.4) - 0.055 };
    (v * 255.0).round() as u8
}

impl From<Rgb> for Oklab {
    fn from(c: Rgb) -> Self {
        let (r, g, b) = (to_linear(c.r), to_linear(c.g), to_linear(c.b));
        let l = (0.412_221_470_8 * r + 0.536_332_536_3 * g + 0.051_445_992_9 * b).cbrt();
        let m = (0.211_903_498_2 * r + 0.680_699_545_1 * g + 0.107_396_956_6 * b).cbrt();
        let s = (0.088_302_461_9 * r + 0.281_718_837_6 * g + 0.629_978_700_5 * b).cbrt();
        Oklab {
            l: 0.210_454_255_3 * l + 0.793_617_785_0 * m - 0.004_072_046_8 * s,
            a: 1.977_998_495_1 * l - 2.428_592_205_0 * m + 0.450_593_709_9 * s,
            b: 0.025_904_037_1 * l + 0.782_771_766_2 * m - 0.808_675_766_0 * s,
        }
    }
}

impl From<Oklab> for Rgb {
    fn from(c: Oklab) -> Self {
        let l = (c.l + 0.396_337_777_4 * c.a + 0.215_803_757_3 * c.b).powi(3);
        let m = (c.l - 0.105_561_345_8 * c.a - 0.063_854_172_8 * c.b).powi(3);
        let s = (c.l - 0.089_484_177_5 * c.a - 1.291_485_548_0 * c.b).powi(3);
        Rgb {
            r: from_linear(4.076_741_662_1 * l - 3.307_711_591_3 * m + 0.230_969_929_2 * s),
            g: from_linear(-1.268_438_004_6 * l + 2.609_757_401_1 * m - 0.341_319_396_5 * s),
            b: from_linear(-0.004_196_086_3 * l - 0.703_418_614_7 * m + 1.707_614_701_0 * s),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_long_and_short_hex() {
        assert_eq!(Rgb::parse("#FF5A1F").unwrap(), Rgb::new(255, 90, 31));
        assert_eq!(Rgb::parse("fff").unwrap(), Rgb::new(255, 255, 255));
        assert_eq!(Rgb::parse("#0a0").unwrap(), Rgb::new(0, 170, 0));
    }

    #[test]
    fn rejects_garbage() {
        assert!(Rgb::parse("#12").is_err());
        assert!(Rgb::parse("#GGGGGG").is_err());
        assert!(Rgb::parse("").is_err());
    }

    #[test]
    fn hex_roundtrip_is_uppercase() {
        assert_eq!(Rgb::parse("#ff5a1f").unwrap().hex(), "#FF5A1F");
        assert_eq!(Rgb::new(1, 2, 3).bare(), "010203");
    }

    #[test]
    fn contrast_extremes() {
        let k = Rgb::new(0, 0, 0);
        let w = Rgb::new(255, 255, 255);
        assert!((k.contrast(w) - 21.0).abs() < 1e-9);
        assert!((w.contrast(w) - 1.0).abs() < 1e-9);
        assert!((k.contrast(w) - w.contrast(k)).abs() < 1e-12);
    }

    #[test]
    fn mix_endpoints_are_exact() {
        let a = Rgb::parse("#FF5A1F").unwrap();
        let b = Rgb::parse("#000000").unwrap();
        assert_eq!(a.mix(b, 0.0), a);
        assert_eq!(a.mix(b, 1.0), b);
    }

    #[test]
    fn mix_midpoint_of_black_and_white_is_perceptual_grey() {
        let m = Rgb::new(0, 0, 0).mix(Rgb::new(255, 255, 255), 0.5);
        // OKLab 0.5 de luminosidade ~ sRGB 99; bem abaixo do 128 de uma mistura ingênua.
        assert!((90..=110).contains(&m.r), "{m}");
        assert_eq!((m.r, m.g), (m.g, m.b));
    }

    #[test]
    fn blend_is_plain_alpha_compositing() {
        let bg = Rgb::new(0, 0, 0);
        let ember = Rgb::parse("#FF5A1F").unwrap();
        assert_eq!(bg.blend(ember, 0.0), bg);
        assert_eq!(bg.blend(ember, 1.0), ember);
        // 18% de laranja sobre preto continua sendo um marrom visível, não preto.
        let tint = bg.blend(ember, 0.18);
        assert_eq!(tint, Rgb::new(46, 16, 6));
        assert!(tint.contrast(bg) > 1.1);
    }

    #[test]
    fn best_of_picks_readable_text_color() {
        let ember = Rgb::parse("#FF5A1F").unwrap();
        let black = Rgb::new(0, 0, 0);
        let white = Rgb::new(255, 255, 255);
        assert_eq!(ember.best_of(black, white), black);
    }
}
