//! Estado persistido do usuário: o que ele escolheu (modo, acento, movimento).

use std::fs;
use std::path::Path;

use std::fmt;
use std::str::FromStr;

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

use crate::fsutil::write_atomic;
use crate::tokens::{Mode, MotionLevel};

/// O jeito do prompt do shell (o starship lê o resultado).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum PromptStyle {
    /// Uma linha: pasta, branch e a seta. O padrão.
    #[default]
    Minimal,
    /// Duas linhas, com versões das linguagens e do ambiente quando o projeto pede.
    Dev,
    /// Só a seta à esquerda; pasta e branch vão para o canto direito.
    Zen,
}

impl PromptStyle {
    pub const ALL: [PromptStyle; 3] = [PromptStyle::Minimal, PromptStyle::Dev, PromptStyle::Zen];

    pub fn id(self) -> &'static str {
        match self {
            PromptStyle::Minimal => "minimal",
            PromptStyle::Dev => "dev",
            PromptStyle::Zen => "zen",
        }
    }

    pub fn blurb(self) -> &'static str {
        match self {
            PromptStyle::Minimal => "uma linha: pasta, branch e a seta",
            PromptStyle::Dev => "duas linhas, com as versões das linguagens do projeto",
            PromptStyle::Zen => "só a seta; pasta e branch ficam no canto direito",
        }
    }

    /// O próximo (ou o anterior, com `delta` negativo), voltando ao começo no fim.
    pub fn step(self, delta: isize) -> Self {
        let i = Self::ALL.iter().position(|p| *p == self).unwrap_or(0) as isize;
        Self::ALL[(i + delta).rem_euclid(Self::ALL.len() as isize) as usize]
    }
}

impl fmt::Display for PromptStyle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.id())
    }
}

impl FromStr for PromptStyle {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> Result<Self> {
        match s.to_ascii_lowercase().as_str() {
            "minimal" | "minimo" | "mínimo" => Ok(PromptStyle::Minimal),
            "dev" => Ok(PromptStyle::Dev),
            "zen" => Ok(PromptStyle::Zen),
            other => bail!("estilo de prompt desconhecido {other:?} (minimal, dev ou zen)"),
        }
    }
}

/// Quando o terminal se apresenta (o resumo do sistema ao abrir).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum GreetMode {
    /// No primeiro terminal depois de ligar, e uma linha discreta na primeira vez que uma workspace vazia recebe um.
    #[default]
    All,
    /// Só no primeiro terminal depois de ligar.
    Boot,
    /// Nunca.
    Off,
}

impl GreetMode {
    pub const ALL: [GreetMode; 3] = [GreetMode::All, GreetMode::Boot, GreetMode::Off];

    pub fn id(self) -> &'static str {
        match self {
            GreetMode::All => "all",
            GreetMode::Boot => "boot",
            GreetMode::Off => "off",
        }
    }

    pub fn step(self, delta: isize) -> Self {
        let i = Self::ALL.iter().position(|p| *p == self).unwrap_or(0) as isize;
        Self::ALL[(i + delta).rem_euclid(Self::ALL.len() as isize) as usize]
    }
}

impl fmt::Display for GreetMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.id())
    }
}

impl FromStr for GreetMode {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> Result<Self> {
        match s.to_ascii_lowercase().as_str() {
            "all" | "tudo" | "on" => Ok(GreetMode::All),
            "boot" | "ligar" => Ok(GreetMode::Boot),
            "off" | "nunca" => Ok(GreetMode::Off),
            other => bail!("modo de saudação desconhecido {other:?} (all, boot ou off)"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(default)]
pub struct State {
    pub mode: Mode,
    /// Nome de um acento dos tokens ou um hex `#RRGGBB`.
    pub accent: String,
    pub motion: MotionLevel,
    /// Id de um estilo procedural (`grade`) ou `file:<nome>` para uma imagem do usuário.
    pub wallpaper: String,
    pub prompt: PromptStyle,
    pub greet: GreetMode,
    /// A proteção de tela: `auto` (reveza entre a marca e os brinquedos instalados), `off`, `marca` ou o id de um brinquedo.
    pub saver: String,
}

impl Default for State {
    fn default() -> Self {
        Self {
            mode: Mode::Dark,
            accent: "ember".into(),
            motion: MotionLevel::Full,
            wallpaper: "grade".into(),
            prompt: PromptStyle::Minimal,
            greet: GreetMode::All,
            saver: "auto".into(),
        }
    }
}

impl State {
    /// Arquivo ausente vira o padrão. Arquivo corrompido é erro (não apagamos escolha do usuário em silêncio).
    pub fn load(path: &Path) -> Result<Self> {
        match fs::read_to_string(path) {
            Ok(s) => toml::from_str(&s).with_context(|| format!("{} está corrompido", path.display())),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(e).with_context(|| format!("lendo {}", path.display())),
        }
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        let body = toml::to_string_pretty(self)?;
        write_atomic(path, body.as_bytes())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("clios-state-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn missing_file_is_default() {
        let d = tmp("missing");
        assert_eq!(State::load(&d.join("state.toml")).unwrap(), State::default());
    }

    #[test]
    fn roundtrip() {
        let d = tmp("roundtrip");
        let s = State {
            mode: Mode::Light,
            accent: "#7CFF00".into(),
            motion: MotionLevel::Reduced,
            wallpaper: "file:praia.jpg".into(),
            prompt: PromptStyle::Zen,
            greet: GreetMode::Boot,
            saver: "bonsai".into(),
        };
        s.save(&d.join("a/b/state.toml")).unwrap();
        assert_eq!(State::load(&d.join("a/b/state.toml")).unwrap(), s);
    }

    #[test]
    fn partial_file_fills_defaults() {
        let d = tmp("partial");
        fs::write(d.join("state.toml"), "accent = \"azure\"\n").unwrap();
        let s = State::load(&d.join("state.toml")).unwrap();
        assert_eq!(s.accent, "azure");
        assert_eq!(s.mode, Mode::Dark);
    }

    #[test]
    fn a_state_file_from_before_v03_still_loads_with_the_new_defaults() {
        let d = tmp("old");
        fs::write(
            d.join("state.toml"),
            "mode = \"light\"\naccent = \"mint\"\nmotion = \"off\"\nwallpaper = \"grade\"\n",
        )
        .unwrap();
        let s = State::load(&d.join("state.toml")).unwrap();
        assert_eq!((s.prompt, s.greet, s.saver.as_str()), (PromptStyle::Minimal, GreetMode::All, "auto"));
    }

    #[test]
    fn prompt_and_greet_steps_wrap_both_ways() {
        assert_eq!(PromptStyle::Zen.step(1), PromptStyle::Minimal);
        assert_eq!(PromptStyle::Minimal.step(-1), PromptStyle::Zen);
        assert_eq!(GreetMode::Off.step(1), GreetMode::All);
        assert_eq!("dev".parse::<PromptStyle>().unwrap(), PromptStyle::Dev);
        assert!("xyz".parse::<PromptStyle>().is_err());
        assert_eq!("nunca".parse::<GreetMode>().unwrap(), GreetMode::Off);
    }

    #[test]
    fn corrupt_file_is_an_error() {
        let d = tmp("corrupt");
        fs::write(d.join("state.toml"), "mode = [[[").unwrap();
        assert!(State::load(&d.join("state.toml")).is_err());
    }
}
