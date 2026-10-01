//! Contexto compartilhado pelos comandos: onde estão as coisas e o que o usuário escolheu.

use std::path::Path;

use anyhow::{Context, Result};
use clios_core::{Paths, State, Theme, Tokens};

pub struct Ctx {
    pub paths: Paths,
    pub tokens: Tokens,
    pub state: State,
    /// `--home` redireciona tudo para outra árvore: é uma simulação (testes, montagem da ISO).
    /// Nunca pode mexer em terminais abertos nem em programas em execução.
    pub sandboxed: bool,
}

impl Ctx {
    pub fn load(root: Option<&Path>, home: Option<&Path>) -> Result<Self> {
        let paths = Paths::discover(root, home)?;
        let src = std::fs::read_to_string(paths.tokens_file())
            .with_context(|| format!("lendo {}", paths.tokens_file().display()))?;
        let tokens = Tokens::parse(&src)?;
        let state = State::load(&paths.state_file())?;
        Ok(Self { paths, tokens, state, sandboxed: home.is_some() })
    }

    pub fn theme(&self) -> Result<Theme> {
        self.theme_for(&self.state)
    }

    pub fn theme_for(&self, state: &State) -> Result<Theme> {
        let mut theme = Theme::resolve(&self.tokens, state.mode, &state.accent, state.motion)?;
        theme.state_dir = self.paths.state.display().to_string();
        Ok(theme)
    }

    pub fn save_state(&self) -> Result<()> {
        self.state.save(&self.paths.state_file())
    }
}
