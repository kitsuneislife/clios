//! O autocompletar do fish sabe os ids do catálogo: `clios open <tab>`, `clios play <tab>`, `clios apps install <tab>`.
//! O clap gera o resto (subcomandos e opções); os ids vêm do catálogo embutido, que é a fonte deles.

use crate::catalog::Catalog;

fn line(sub: &[&str], ids: &[String]) -> String {
    let cond = sub.iter().map(|s| format!("__fish_seen_subcommand_from {s}")).collect::<Vec<_>>().join("; and ");
    format!("complete -c clios -f -n '{cond}' -a '{}'\n", ids.join(" "))
}

pub fn fish_ids(cat: &Catalog) -> String {
    let all: Vec<String> = cat.tui.iter().map(|t| t.id.clone()).collect();
    let mut open = vec!["hub".to_string()];
    open.extend(all.iter().cloned());
    let toys: Vec<String> = cat.tui.iter().filter(|t| !t.saver.is_empty()).map(|t| t.id.clone()).collect();
    let mut scenes: Vec<String> = ["auto", "marca", "off"].map(String::from).to_vec();
    scenes.extend(toys.iter().cloned());

    let mut out = String::from("\n# ids do catálogo (gerado por `clios completions fish`)\n");
    out.push_str(&line(&["open"], &open));
    out.push_str(&line(&["play"], &toys));
    out.push_str(&line(&["apps"], &all));
    out.push_str(&format!(
        "complete -c clios -f -n '__fish_seen_subcommand_from saver; and __fish_seen_subcommand_from set' -a '{}'\n",
        scenes.join(" ")
    ));
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::builtin;

    #[test]
    fn fish_gets_every_catalog_id_for_open_and_only_toys_for_play() {
        let out = fish_ids(&builtin());
        let open = out.lines().find(|l| l.contains("seen_subcommand_from open")).unwrap();
        for id in ["hub", "files", "git", "bonsai", "notifs"] {
            assert!(open.contains(&format!(" {id}")) || open.contains(&format!("'{id} ")), "{id}: {open}");
        }
        let play = out.lines().find(|l| l.contains("seen_subcommand_from play")).unwrap();
        assert!(play.contains("bonsai") && !play.contains("files"), "{play}");
        assert!(out.contains("saver; and __fish_seen_subcommand_from set"));
    }
}
