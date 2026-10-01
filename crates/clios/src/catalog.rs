//! O catálogo de apps curados: o que o CLIOS recomenda, como abre e como instala.
//!
//! Vem de `config/clios/hub.toml` (embutido no binário). O usuário pode sobrescrever copiando o
//! arquivo para `~/.config/clios/hub.toml`. O hub, o `clios apps` e o welcome leem o mesmo catálogo.

use clios_core::Paths;
use serde::Deserialize;

use crate::sys::{is_installed, sh_quote};

pub const DEFAULT_CATALOG: &str = include_str!("../../../config/clios/hub.toml");

/// Categorias, na ordem em que aparecem. O id é o que o catálogo usa.
pub const CATEGORIES: &[(&str, &str)] = &[
    ("arquivos", "arquivos"),
    ("codigo", "código"),
    ("sistema", "sistema"),
    ("rede", "rede"),
    ("midia", "mídia"),
    ("ler", "ler"),
    ("falar", "conversar"),
    ("produtividade", "produtividade"),
    ("pacotes", "pacotes"),
    ("diversao", "diversão"),
];

pub fn category_name(id: &str) -> &str {
    CATEGORIES.iter().find(|(i, _)| *i == id).map_or(id, |(_, n)| n)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Tier {
    /// Vem instalado pelo bootstrap.
    Core,
    /// Recomendado; instala sob demanda (hub `+`, `clios apps install`).
    #[default]
    Extra,
}

#[derive(Debug, Default, Deserialize)]
pub struct Catalog {
    #[serde(default)]
    pub tui: Vec<TuiDef>,
    #[serde(default)]
    pub cmd: Vec<CmdDef>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TuiDef {
    pub id: String,
    pub name: String,
    pub cmd: Vec<String>,
    #[serde(default)]
    pub keywords: String,
    #[serde(default)]
    pub float: bool,
    #[serde(default)]
    pub hold: bool,
    /// Pacote(s) do Arch/AUR, separados por espaço. Vazio: não há o que instalar.
    #[serde(default)]
    pub pkg: String,
    /// Executável que prova que está instalado (padrão: o primeiro item de `cmd`).
    #[serde(default)]
    pub bin: Option<String>,
    #[serde(default = "default_category")]
    pub category: String,
    /// Uma frase em português sobre o que o app faz.
    #[serde(default)]
    pub desc: String,
    /// Uma dica de uso: como sair, a tecla mais útil.
    #[serde(default)]
    pub tip: String,
    #[serde(default)]
    pub tier: Tier,
}

fn default_category() -> String {
    "sistema".into()
}

#[derive(Debug, Clone, Deserialize)]
pub struct CmdDef {
    pub id: String,
    pub name: String,
    pub shell: String,
    #[serde(default)]
    pub keywords: String,
}

impl TuiDef {
    pub fn bin(&self) -> &str {
        self.bin.as_deref().unwrap_or_else(|| self.cmd.first().map_or("", String::as_str))
    }

    pub fn installed(&self) -> bool {
        !self.bin().is_empty() && is_installed(self.bin())
    }

    /// O comando que instala o app: `paru` resolve repositórios e AUR com o mesmo comando;
    /// sem `paru`, cai para `pacman` (só repositórios oficiais).
    pub fn install_argv(&self) -> Vec<String> {
        install_argv(&self.pkg)
    }
}

pub fn install_argv(pkg: &str) -> Vec<String> {
    let pkgs = pkg.split_whitespace().map(sh_quote).collect::<Vec<_>>().join(" ");
    vec![
        "sh".into(),
        "-c".into(),
        format!(
            "if command -v paru >/dev/null 2>&1; then paru -S --needed {pkgs}; \
             else sudo pacman -S --needed {pkgs}; fi"
        ),
    ]
}

/// O catálogo do usuário se existir e for válido; senão o embutido. O aviso diz por quê.
pub fn load(paths: &Paths) -> (Catalog, Option<String>) {
    let user = paths.config.join("clios/hub.toml");
    if let Ok(text) = std::fs::read_to_string(&user) {
        match toml::from_str::<Catalog>(&text) {
            Ok(c) => return (c, None),
            Err(e) => {
                let first = e.to_string().lines().next().unwrap_or_default().to_string();
                return (builtin(), Some(format!("hub.toml inválido, usando o padrão: {first}")));
            }
        }
    }
    (builtin(), None)
}

pub fn builtin() -> Catalog {
    toml::from_str(DEFAULT_CATALOG).expect("catálogo embutido precisa ser válido")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn embedded_catalog_parses_and_is_big_enough() {
        let c = builtin();
        assert!(c.tui.len() >= 40, "{} entradas", c.tui.len());
    }

    #[test]
    fn ids_are_unique_across_tuis_and_cmds() {
        let c = builtin();
        let mut seen = HashSet::new();
        for id in c.tui.iter().map(|t| &t.id).chain(c.cmd.iter().map(|c| &c.id)) {
            assert!(seen.insert(id.clone()), "id duplicado: {id}");
        }
    }

    #[test]
    fn every_entry_is_complete() {
        let known: HashSet<&str> = CATEGORIES.iter().map(|(i, _)| *i).collect();
        for t in &builtin().tui {
            assert!(!t.cmd.is_empty(), "{}: sem comando", t.id);
            assert!(known.contains(t.category.as_str()), "{}: categoria desconhecida {:?}", t.id, t.category);
            assert!(!t.desc.is_empty(), "{}: sem descrição", t.id);
            assert!(t.desc.chars().count() <= 90, "{}: descrição longa demais ({})", t.id, t.desc.chars().count());
            assert!(!t.name.is_empty() && t.name == t.name.to_lowercase(), "{}: o nome é em minúsculas", t.id);
            assert!(!t.pkg.is_empty(), "{}: sem pacote", t.id);
            assert!(!t.desc.ends_with('.'), "{}: descrição sem ponto final, como o resto da interface", t.id);
        }
    }

    #[test]
    fn every_category_has_at_least_two_apps() {
        let c = builtin();
        for (id, _) in CATEGORIES {
            let n = c.tui.iter().filter(|t| t.category == *id).count();
            assert!(n >= 2, "categoria {id} tem só {n}");
        }
    }

    #[test]
    fn bin_defaults_to_the_first_word_of_the_command() {
        let c = builtin();
        let yazi = c.tui.iter().find(|t| t.id == "files").unwrap();
        assert_eq!(yazi.bin(), "yazi");
        let trip = c.tui.iter().find(|t| t.id == "route").unwrap();
        assert_eq!(trip.bin(), "trip", "o binário do trippy não é o nome do pacote");
    }

    #[test]
    fn install_prefers_paru_and_quotes_every_package() {
        let a = install_argv("taskwarrior-tui task");
        assert_eq!(&a[..2], ["sh", "-c"]);
        assert!(a[2].contains("paru -S --needed taskwarrior-tui task"), "{}", a[2]);
        assert!(a[2].contains("sudo pacman -S --needed taskwarrior-tui task"));
        let evil = install_argv("x; rm -rf ~");
        assert!(evil[2].contains("'x;' rm -rf"), "{}", evil[2]);
        assert!(!evil[2].contains("needed x; rm"), "o ponto e vírgula não pode virar um comando novo");
    }

    #[test]
    fn user_catalog_that_is_broken_falls_back_with_a_reason() {
        let d = std::env::temp_dir().join(format!("clios-cat-{}", std::process::id()));
        std::fs::create_dir_all(d.join("tokens")).unwrap();
        std::fs::write(d.join("tokens/tokens.toml"), "").unwrap();
        let paths = Paths::discover(Some(&d), Some(&d)).unwrap();
        std::fs::create_dir_all(paths.config.join("clios")).unwrap();
        std::fs::write(paths.config.join("clios/hub.toml"), "[[tui]]\nid = 1\n").unwrap();
        let (c, warn) = load(&paths);
        assert!(c.tui.len() >= 40);
        assert!(warn.unwrap().contains("hub.toml inválido"));
    }
}

#[cfg(test)]
mod package_lists {
    use super::*;

    const OFFICIAL: &str = include_str!("../../../packages/official.txt");
    const AUR: &str = include_str!("../../../packages/aur.txt");

    fn listed() -> Vec<&'static str> {
        OFFICIAL
            .lines()
            .chain(AUR.lines())
            .map(|l| l.split('#').next().unwrap().trim())
            .filter(|l| !l.is_empty())
            .collect()
    }

    #[test]
    fn every_core_app_is_installed_by_the_bootstrap() {
        let listed = listed();
        for t in builtin().tui.iter().filter(|t| t.tier == Tier::Core) {
            for p in t.pkg.split_whitespace().filter(|p| *p != "clios") {
                assert!(listed.contains(&p), "{}: o pacote {p} é core mas não está em packages/*.txt", t.id);
            }
        }
    }

    #[test]
    fn extras_are_left_for_on_demand_install() {
        let listed = listed();
        for t in builtin().tui.iter().filter(|t| t.tier == Tier::Extra) {
            for p in t.pkg.split_whitespace() {
                assert!(!listed.contains(&p), "{}: {p} é extra, não pode estar no bootstrap base", t.id);
            }
        }
    }
}
