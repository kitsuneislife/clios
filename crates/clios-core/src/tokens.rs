//! Esquema de `tokens/tokens.toml`: o que o designer escreve.

use std::collections::BTreeMap;
use std::fmt;
use std::str::FromStr;

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

use crate::color::Rgb;

pub const DEFAULT_TOKENS: &str = include_str!("../../../tokens/tokens.toml");

#[derive(Debug, Clone, Deserialize)]
pub struct Tokens {
    pub font: Font,
    pub ui: Ui,
    pub motion: MotionTokens,
    pub mode: Modes,
    pub accent: BTreeMap<String, AccentDef>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Font {
    pub mono: String,
    pub size: f64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Ui {
    pub gap_in: u32,
    pub gap_out: u32,
    pub border: u32,
    pub radius: u32,
    pub bar_height: u32,
    pub text: u32,
    pub text_small: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MotionTokens {
    pub duration: Durations,
    pub curve: BTreeMap<String, CurveDef>,
    pub spring: BTreeMap<String, SpringDef>,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
pub struct Durations {
    pub instant: u32,
    pub fast: u32,
    pub base: u32,
    pub slow: u32,
}

#[derive(Debug, Clone, Copy, Deserialize)]
pub struct CurveDef {
    pub points: [f64; 4],
}

#[derive(Debug, Clone, Copy, Deserialize)]
pub struct SpringDef {
    pub mass: f64,
    pub stiffness: f64,
    pub damping_ratio: f64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Modes {
    pub dark: Palette,
    pub light: Palette,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Palette {
    pub bg: Rgb,
    pub surface: Rgb,
    pub raised: Rgb,
    pub line: Rgb,
    pub mute: Rgb,
    pub dim: Rgb,
    pub fg: Rgb,
    pub ansi: Ansi,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Ansi {
    pub black: Rgb,
    pub red: Rgb,
    pub green: Rgb,
    pub yellow: Rgb,
    pub blue: Rgb,
    pub magenta: Rgb,
    pub cyan: Rgb,
    pub white: Rgb,
}

#[derive(Debug, Clone, Copy, Deserialize)]
pub struct AccentDef {
    pub dark: Rgb,
    pub light: Rgb,
}

impl Tokens {
    pub fn parse(src: &str) -> Result<Self> {
        toml::from_str(src).context("tokens.toml inválido")
    }

    pub fn builtin() -> Self {
        Self::parse(DEFAULT_TOKENS).expect("tokens embutidos precisam ser válidos")
    }

    pub fn palette(&self, mode: Mode) -> &Palette {
        match mode {
            Mode::Dark => &self.mode.dark,
            Mode::Light => &self.mode.light,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    #[default]
    Dark,
    Light,
}

impl Mode {
    pub fn toggled(self) -> Self {
        match self {
            Mode::Dark => Mode::Light,
            Mode::Light => Mode::Dark,
        }
    }
}

impl fmt::Display for Mode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Mode::Dark => "dark",
            Mode::Light => "light",
        })
    }
}

impl FromStr for Mode {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> Result<Self> {
        match s.to_ascii_lowercase().as_str() {
            "dark" | "escuro" => Ok(Mode::Dark),
            "light" | "claro" => Ok(Mode::Light),
            other => bail!("modo desconhecido {other:?} (use dark ou light)"),
        }
    }
}

/// Quanto movimento o usuário quer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum MotionLevel {
    /// Movimento completo: molas, deslizes, fades.
    #[default]
    Full,
    /// Só fades curtos. Nada desliza, nada escala.
    Reduced,
    /// Tudo instantâneo.
    Off,
}

impl MotionLevel {
    pub fn cycled(self) -> Self {
        match self {
            MotionLevel::Full => MotionLevel::Reduced,
            MotionLevel::Reduced => MotionLevel::Off,
            MotionLevel::Off => MotionLevel::Full,
        }
    }
}

impl fmt::Display for MotionLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            MotionLevel::Full => "full",
            MotionLevel::Reduced => "reduced",
            MotionLevel::Off => "off",
        })
    }
}

impl FromStr for MotionLevel {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> Result<Self> {
        match s.to_ascii_lowercase().as_str() {
            "full" | "on" => Ok(MotionLevel::Full),
            "reduced" | "reduzido" => Ok(MotionLevel::Reduced),
            "off" | "none" => Ok(MotionLevel::Off),
            other => bail!("nível de movimento desconhecido {other:?} (full, reduced ou off)"),
        }
    }
}
