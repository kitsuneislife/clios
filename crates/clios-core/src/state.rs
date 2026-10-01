//! Estado persistido do usuário: o que ele escolheu (modo, acento, movimento).

use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::fsutil::write_atomic;
use crate::tokens::{Mode, MotionLevel};

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(default)]
pub struct State {
    pub mode: Mode,
    /// Nome de um acento dos tokens ou um hex `#RRGGBB`.
    pub accent: String,
    pub motion: MotionLevel,
    /// Id de um estilo procedural (`grade`) ou `file:<nome>` para uma imagem do usuário.
    pub wallpaper: String,
}

impl Default for State {
    fn default() -> Self {
        Self { mode: Mode::Dark, accent: "ember".into(), motion: MotionLevel::Full, wallpaper: "grade".into() }
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
    fn corrupt_file_is_an_error() {
        let d = tmp("corrupt");
        fs::write(d.join("state.toml"), "mode = [[[").unwrap();
        assert!(State::load(&d.join("state.toml")).is_err());
    }
}
