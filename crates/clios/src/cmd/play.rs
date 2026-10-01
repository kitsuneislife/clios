//! `clios play`: um brinquedo do catálogo, aqui mesmo, na cor do seu acento.

use anyhow::{Result, bail};

use crate::catalog::{self, TuiDef};
use crate::ctx::Ctx;
use crate::{toys, ui};

/// Sorteia entre os instalados, sem repetir o de antes quando há escolha.
pub fn choose<'a>(installed: &[&'a TuiDef], seed: usize) -> Option<&'a TuiDef> {
    if installed.is_empty() {
        return None;
    }
    installed.get(seed % installed.len()).copied()
}

pub fn run(ctx: &Ctx, id: Option<&str>, list: bool) -> Result<bool> {
    let (cat, _) = catalog::load(&ctx.paths);
    let toys_all: Vec<&TuiDef> = cat.tui.iter().filter(|t| !t.saver.is_empty()).collect();
    if list {
        for t in &toys_all {
            let tag = if t.installed() {
                String::new()
            } else {
                ui::dim("  (clios apps install ") + &ui::dim(&format!("{})", t.id))
            };
            println!("{:<12} {}{tag}", t.id, t.desc);
        }
        return Ok(true);
    }
    let theme = ctx.theme()?;
    let toy = match id {
        Some(id) => {
            let Some(t) = toys_all.iter().find(|t| t.id == id) else {
                let ids: Vec<&str> = toys_all.iter().map(|t| t.id.as_str()).collect();
                bail!("{id:?} não é um brinquedo. Os que existem: {}", ids.join(", "));
            };
            if !t.installed() {
                bail!("{} não está instalado. Instale com: clios apps install {}", t.name, t.id);
            }
            *t
        }
        None => {
            let installed = toys::installed_scenes(&cat.tui);
            let seed = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.subsec_nanos() as usize);
            let Some(t) = choose(&installed, seed) else {
                bail!("nenhum brinquedo instalado. Instale um: clios apps install bonsai");
            };
            t
        }
    };
    let argv = toy.scene_argv(&theme).expect("só brinquedos têm saver");
    let ok = std::process::Command::new(&argv[0]).args(&argv[1..]).status().is_ok_and(|s| s.success());
    Ok(ok)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn choose_wraps_and_handles_nothing_installed() {
        let cat = catalog::builtin();
        let all: Vec<&TuiDef> = cat.tui.iter().filter(|t| !t.saver.is_empty()).collect();
        assert!(choose(&[], 3).is_none());
        assert_eq!(choose(&all, 0).unwrap().id, all[0].id);
        assert_eq!(choose(&all, all.len()).unwrap().id, all[0].id);
    }
}
