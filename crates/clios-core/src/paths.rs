//! Onde as coisas ficam.
//!
//! `root`   o checkout do CLIOS (tokens/, templates/, config/).
//! `config` o equivalente a ~/.config.
//! `state`  ~/.local/state/clios (estado do usuário e arquivos gerados que não são dotfiles).

use std::env;
use std::path::{Path, PathBuf};

use anyhow::{Result, bail};

#[derive(Debug, Clone)]
pub struct Paths {
    pub root: PathBuf,
    pub home: PathBuf,
    pub config: PathBuf,
    pub state: PathBuf,
    /// ~/.local/share/clios: coisas do usuário que não são config nem estado (papéis de parede dele).
    pub data: PathBuf,
}

impl Paths {
    /// `home_override` redireciona tudo para outra árvore (usado para montar a ISO e nos testes).
    pub fn discover(root: Option<&Path>, home_override: Option<&Path>) -> Result<Self> {
        let root = find_root(root)?;
        let (home, config, state, data) = match home_override {
            Some(h) => (h.to_path_buf(), h.join(".config"), h.join(".local/state/clios"), h.join(".local/share/clios")),
            None => {
                let home = PathBuf::from(env::var_os("HOME").unwrap_or_default());
                let xdg = |var: &str, fallback: &str| {
                    env::var_os(var)
                        .map(PathBuf::from)
                        .filter(|p| p.is_absolute())
                        .unwrap_or_else(|| home.join(fallback))
                };
                (
                    home.clone(),
                    xdg("XDG_CONFIG_HOME", ".config"),
                    xdg("XDG_STATE_HOME", ".local/state").join("clios"),
                    xdg("XDG_DATA_HOME", ".local/share").join("clios"),
                )
            }
        };
        Ok(Self { root, home, config, state, data })
    }

    /// ~/.local/share (o pai de `data`): onde o xdg procura os .desktop do usuário.
    pub fn xdg_data_home(&self) -> PathBuf {
        self.data.parent().map_or_else(|| self.home.join(".local/share"), Path::to_path_buf)
    }

    pub fn tokens_file(&self) -> PathBuf {
        self.root.join("tokens/tokens.toml")
    }

    pub fn templates_dir(&self) -> PathBuf {
        self.root.join("templates")
    }

    pub fn static_dir(&self) -> PathBuf {
        self.root.join("config")
    }

    /// Arquivos que o app reescreve (btop, por exemplo): copiados uma vez, nunca ligados nem sobrescritos.
    pub fn seed_dir(&self) -> PathBuf {
        self.root.join("seed")
    }

    pub fn wallpapers_dir(&self) -> PathBuf {
        self.data.join("wallpapers")
    }

    pub fn state_file(&self) -> PathBuf {
        self.state.join("state.toml")
    }

    /// Resolve um destino de manifesto: `@state/x` vai para o estado, o resto para ~/.config.
    pub fn resolve_dst(&self, dst: &str) -> PathBuf {
        match dst.strip_prefix("@state/") {
            Some(rest) => self.state.join(rest),
            None => self.config.join(dst),
        }
    }
}

fn is_root(p: &Path) -> bool {
    p.join("tokens/tokens.toml").is_file()
}

fn find_root(explicit: Option<&Path>) -> Result<PathBuf> {
    // Quem pediu um diretório específico recebe erro se ele não servir; nada de fallback silencioso.
    let pinned = explicit.map(Path::to_path_buf).or_else(|| env::var_os("CLIOS_ROOT").map(PathBuf::from));
    if let Some(p) = pinned {
        if is_root(&p) {
            return Ok(p);
        }
        bail!("{} não é um diretório do CLIOS (falta tokens/tokens.toml)", p.display());
    }

    let mut candidates: Vec<PathBuf> = Vec::new();
    // Rodando de um checkout: .../clios/target/release/clios
    if let Ok(exe) = env::current_exe() {
        candidates.extend(exe.ancestors().skip(1).take(4).map(Path::to_path_buf));
    }
    if let Some(home) = env::var_os("HOME") {
        candidates.push(Path::new(&home).join(".local/share/clios"));
    }
    candidates.push(PathBuf::from("/usr/share/clios"));

    if let Some(found) = candidates.iter().find(|c| is_root(c)) {
        return Ok(found.clone());
    }
    let list: Vec<String> = candidates.iter().map(|p| format!("  {}", p.display())).collect();
    bail!(
        "não achei o diretório do CLIOS (precisa conter tokens/tokens.toml). Procurei em:\n{}\nUse --root ou CLIOS_ROOT.",
        list.join("\n")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dst_prefixes() {
        let p = Paths {
            root: "/r".into(),
            home: "/h".into(),
            config: "/h/.config".into(),
            state: "/h/.local/state/clios".into(),
            data: "/h/.local/share/clios".into(),
        };
        assert_eq!(p.resolve_dst("hypr/theme.lua"), PathBuf::from("/h/.config/hypr/theme.lua"));
        assert_eq!(p.resolve_dst("@state/theme.json"), PathBuf::from("/h/.local/state/clios/theme.json"));
    }

    #[test]
    fn explicit_root_must_contain_tokens() {
        let dir = tempdir();
        assert!(Paths::discover(Some(&dir), Some(&dir)).is_err_and(|e| e.to_string().contains("tokens.toml")));
    }

    fn tempdir() -> PathBuf {
        let d = env::temp_dir().join(format!("clios-paths-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        d
    }
}
