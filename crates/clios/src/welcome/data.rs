//! O conteúdo do guia: atalhos e dicas (arquivos TOML no repositório) e os primeiros passos.

use clios_core::State;
use serde::Deserialize;

const KEYS: &str = include_str!("../../../../config/clios/keys.toml");
const TIPS: &str = include_str!("../../../../config/clios/tips.toml");

#[derive(Debug, Clone, Deserialize)]
pub struct Key {
    pub keys: String,
    #[allow(dead_code)] // conferido pelo teste do Hyprland (tests/hypr/check_config.py)
    pub bind: Vec<String>,
    pub what: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Group {
    #[allow(dead_code)]
    pub id: String,
    pub name: String,
    pub blurb: String,
    pub key: Vec<Key>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Tip {
    pub title: String,
    pub body: String,
    #[serde(default)]
    pub keys: Option<String>,
}

#[derive(Deserialize)]
struct KeyFile {
    group: Vec<Group>,
}

#[derive(Deserialize)]
struct TipFile {
    tip: Vec<Tip>,
}

pub fn groups() -> Vec<Group> {
    toml::from_str::<KeyFile>(KEYS).expect("keys.toml embutido precisa ser válido").group
}

pub fn tips() -> Vec<Tip> {
    toml::from_str::<TipFile>(TIPS).expect("tips.toml embutido precisa ser válido").tip
}

/// Uma coisa a fazer para sentir-se em casa.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Step {
    pub title: &'static str,
    pub keys: &'static str,
    /// `None`: é só uma sugestão, não dá para saber se já foi feito.
    pub done: Option<bool>,
}

/// O que dá para observar no sistema (separado dos passos para os passos serem testáveis).
#[derive(Debug, Clone, Default)]
pub struct Probes {
    pub hub_used: bool,
    pub online: bool,
    pub aur_helper: bool,
    pub extras_missing: usize,
}

pub fn steps(state: &State, p: &Probes) -> Vec<Step> {
    vec![
        Step { title: "abra o hub e procure qualquer coisa", keys: "super + espaço", done: Some(p.hub_used) },
        Step { title: "escolha o seu acento", keys: "super + F2", done: Some(state.accent != "ember") },
        Step { title: "escolha um papel de parede", keys: "super + F4", done: Some(state.wallpaper != "grade") },
        Step { title: "conecte-se à rede", keys: "super + i", done: Some(p.online) },
        Step {
            title: "instale os apps recomendados",
            keys: "clios apps install --extras",
            done: Some(p.aur_helper && p.extras_missing == 0),
        },
        Step { title: "veja todos os atalhos", keys: "super + /", done: None },
        Step { title: "espaireça com um brinquedo", keys: "super + z", done: None },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn embedded_files_parse_and_have_content() {
        let g = groups();
        assert!(g.len() >= 5);
        assert!(g.iter().all(|g| !g.key.is_empty() && !g.name.is_empty() && !g.blurb.is_empty()));
        assert!(tips().len() >= 15);
    }

    #[test]
    fn every_key_entry_is_complete_and_unique() {
        let mut seen = HashSet::new();
        for g in groups() {
            for k in &g.key {
                assert!(!k.bind.is_empty(), "{}: sem bind real", k.keys);
                assert!(!k.what.ends_with('.'), "{}: sem ponto final", k.what);
                assert!(k.what.chars().count() <= 44, "{}: cabe mal na coluna da página de atalhos", k.what);
                assert!(seen.insert(k.keys.clone()), "{} repetido", k.keys);
            }
        }
    }

    #[test]
    fn tips_are_short_and_distinct() {
        let t = tips();
        let titles: HashSet<_> = t.iter().map(|t| t.title.clone()).collect();
        assert_eq!(titles.len(), t.len());
        for tip in &t {
            assert!(tip.body.chars().count() <= 220, "{}: corpo longo demais", tip.title);
            assert!(tip.title.chars().count() <= 40, "{}: título longo demais", tip.title);
        }
    }

    #[test]
    fn steps_follow_the_state() {
        let mut s = State::default();
        let none = steps(&s, &Probes::default());
        assert!(none.iter().filter(|x| x.done == Some(true)).count() == 0);
        s.accent = "azure".into();
        s.wallpaper = "aneis".into();
        let p = Probes { hub_used: true, online: true, aur_helper: true, extras_missing: 0 };
        let all = steps(&s, &p);
        assert!(all.iter().all(|x| x.done != Some(false)), "{all:?}");
        let p2 = Probes { aur_helper: true, extras_missing: 3, ..p };
        assert_eq!(steps(&s, &p2)[4].done, Some(false), "faltam extras");
        let p3 = Probes { aur_helper: false, extras_missing: 0, ..p };
        assert_eq!(steps(&s, &p3)[4].done, Some(false), "sem o paru não dá para instalar");
    }
}
