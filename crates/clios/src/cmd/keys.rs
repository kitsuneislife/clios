//! `clios keys`: os atalhos curados no terminal, para consultar sem sair do teclado (ou por ssh).
//! Vem do mesmo `keys.toml` do guia de boas-vindas, então nunca diverge dele.

use anyhow::Result;

use crate::ui;
use crate::welcome::data::{self, Group};

/// Os grupos que sobram depois do filtro. Casa com o nome do grupo, a tecla ou a descrição, sem ligar para acento nem caixa.
pub fn filter(groups: &[Group], query: &str) -> Vec<Group> {
    let norm = |s: &str| {
        s.to_lowercase()
            .chars()
            .map(|c| match c {
                'á' | 'à' | 'â' | 'ã' => 'a',
                'é' | 'ê' => 'e',
                'í' => 'i',
                'ó' | 'ô' | 'õ' => 'o',
                'ú' => 'u',
                'ç' => 'c',
                c => c,
            })
            .collect::<String>()
    };
    let q = norm(query.trim());
    if q.is_empty() {
        return groups.to_vec();
    }
    groups
        .iter()
        .filter_map(|g| {
            if norm(&g.name).contains(&q) {
                return Some(g.clone());
            }
            let key: Vec<_> =
                g.key.iter().filter(|k| norm(&k.keys).contains(&q) || norm(&k.what).contains(&q)).cloned().collect();
            (!key.is_empty()).then(|| Group { key, ..g.clone() })
        })
        .collect()
}

pub fn run(query: &str) -> Result<()> {
    let groups = filter(&data::groups(), query);
    if groups.is_empty() {
        println!("nenhum atalho combina com {query:?}");
        return Ok(());
    }
    let width = groups.iter().flat_map(|g| &g.key).map(|k| k.keys.chars().count()).max().unwrap_or(0);
    for (i, g) in groups.iter().enumerate() {
        if i > 0 {
            println!();
        }
        println!("{}  {}", ui::bold(&g.name), ui::dim(&g.blurb));
        for k in &g.key {
            println!("  {:<width$}  {}", k.keys, ui::dim(&k.what));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_empty_query_keeps_everything() {
        assert_eq!(filter(&data::groups(), "  ").len(), data::groups().len());
    }

    #[test]
    fn it_matches_group_names_keys_and_descriptions_without_caring_about_accents() {
        let g = data::groups();
        let by_name = filter(&g, "capturas");
        assert_eq!(by_name.len(), 1);
        assert_eq!(
            by_name[0].key.len(),
            g.iter().find(|x| x.name.starts_with("capturas")).unwrap().key.len(),
            "o grupo inteiro"
        );
        let by_desc = filter(&g, "area de transferencia");
        assert!(by_desc.iter().flat_map(|x| &x.key).any(|k| k.keys == "super + v"), "{by_desc:?}");
        let by_key = filter(&g, "shift + s");
        assert!(by_key.iter().flat_map(|x| &x.key).any(|k| k.keys == "super + shift + s"));
        assert!(filter(&g, "xyzzy").is_empty());
    }
}
