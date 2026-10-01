//! `clios apps`: o catálogo de apps curados, no terminal.

use std::process::Command;

use anyhow::{Result, bail};

use crate::catalog::{CATEGORIES, Catalog, Tier, TuiDef, category_name};
use crate::ctx::Ctx;
use crate::sys::is_installed;
use crate::ui;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Filter {
    All,
    Installed,
    Missing,
}

pub fn list(ctx: &Ctx, filter: Filter, category: Option<&str>) -> Result<()> {
    let (catalog, warn) = crate::catalog::load(&ctx.paths);
    if let Some(w) = warn {
        eprintln!("aviso: {w}");
    }
    if let Some(c) = category {
        if !CATEGORIES.iter().any(|(id, name)| *id == c || *name == c) {
            let names: Vec<&str> = CATEGORIES.iter().map(|(id, _)| *id).collect();
            bail!("categoria {c:?} não existe. Use: {}", names.join(", "));
        }
    }
    let (mut have, mut total) = (0, 0);
    for (cat_id, cat_name) in CATEGORIES {
        if category.is_some_and(|c| c != *cat_id && c != *cat_name) {
            continue;
        }
        let rows: Vec<&TuiDef> = catalog
            .tui
            .iter()
            .filter(|t| t.category == *cat_id)
            .filter(|t| match filter {
                Filter::All => true,
                Filter::Installed => t.installed(),
                Filter::Missing => !t.installed(),
            })
            .collect();
        if rows.is_empty() {
            continue;
        }
        println!("{}", ui::bold(cat_name));
        for t in rows {
            let ok = t.installed();
            have += usize::from(ok);
            total += 1;
            let mark = if ok { "✓" } else { "·" };
            let name = format!("{:<16}", t.name);
            println!("  {mark} {} {}", if ok { name } else { ui::dim(&name) }, ui::dim(&t.desc));
        }
        println!();
    }
    println!(
        "{}",
        ui::dim(&format!(
            "{have} de {total} instalados. Instalar: clios apps install <id>  ·  ids em ~/.config/clios/hub.toml"
        ))
    );
    Ok(())
}

/// Os pacotes a instalar, sem repetir e sem os que já existem. Erra se um id não existir.
pub fn plan_install(
    catalog: &Catalog,
    ids: &[String],
    extras: bool,
    installed: &dyn Fn(&TuiDef) -> bool,
) -> Result<Vec<String>> {
    let mut pkgs: Vec<String> = Vec::new();
    let mut add = |t: &TuiDef| {
        for p in t.pkg.split_whitespace().filter(|p| *p != "clios") {
            if !pkgs.iter().any(|x| x == p) {
                pkgs.push(p.to_string());
            }
        }
    };
    for id in ids {
        let Some(t) = catalog.tui.iter().find(|t| &t.id == id) else {
            let all: Vec<&str> = catalog.tui.iter().map(|t| t.id.as_str()).collect();
            bail!("não há app {id:?} no catálogo. Ids: {}", all.join(", "));
        };
        add(t);
    }
    if extras {
        for t in catalog.tui.iter().filter(|t| t.tier == Tier::Extra && !installed(t)) {
            add(t);
        }
    }
    Ok(pkgs)
}

pub fn install(ctx: &Ctx, ids: &[String], extras: bool, dry_run: bool) -> Result<bool> {
    let (catalog, _) = crate::catalog::load(&ctx.paths);
    let pkgs = plan_install(&catalog, ids, extras, &|t| t.installed())?;
    if pkgs.is_empty() {
        println!("nada a instalar: tudo isso já está instalado.");
        return Ok(true);
    }
    let helper = if is_installed("paru") {
        Some("paru")
    } else if is_installed("yay") {
        Some("yay")
    } else {
        None
    };
    let mut argv: Vec<String> = match helper {
        Some(h) => vec![h.into(), "-S".into(), "--needed".into()],
        None => vec!["sudo".into(), "pacman".into(), "-S".into(), "--needed".into()],
    };
    argv.extend(pkgs.iter().cloned());
    if helper.is_none() {
        eprintln!("{}", ui::dim("sem paru ou yay: só os pacotes dos repositórios oficiais vão funcionar"));
    }
    println!("{}", ui::dim(&format!("$ {}", argv.join(" "))));
    if dry_run {
        return Ok(true);
    }
    Ok(Command::new(&argv[0]).args(&argv[1..]).status()?.success())
}

pub fn info(ctx: &Ctx, id: &str) -> Result<()> {
    let (catalog, _) = crate::catalog::load(&ctx.paths);
    let Some(t) = catalog.tui.iter().find(|t| t.id == id) else {
        bail!("não há app {id:?} no catálogo");
    };
    println!(
        "{}  {}",
        ui::bold(&t.name),
        ui::dim(&format!(
            "({}, {})",
            category_name(&t.category),
            if t.installed() { "instalado" } else { "não instalado" }
        ))
    );
    println!("{}", t.desc);
    if !t.tip.is_empty() {
        println!("{} {}", ui::dim("dica:"), t.tip);
    }
    println!("{} clios open {}", ui::dim("abrir:"), t.id);
    println!("{} {}", ui::dim("pacote:"), t.pkg);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::builtin;

    #[test]
    fn plan_for_ids_dedups_and_splits_multi_package_entries() {
        let c = builtin();
        let p = plan_install(&c, &["tasks".into(), "tasks".into(), "visualizer".into()], false, &|_| false).unwrap();
        assert_eq!(p, ["taskwarrior-tui", "task", "cava"]);
    }

    #[test]
    fn unknown_id_lists_what_exists() {
        let err = plan_install(&builtin(), &["banana".into()], false, &|_| false).unwrap_err().to_string();
        assert!(err.contains("banana") && err.contains("files"), "{err}");
    }

    #[test]
    fn extras_skip_what_is_installed_and_never_include_core() {
        let c = builtin();
        let none = plan_install(&c, &[], true, &|_| false).unwrap();
        assert!(none.contains(&"cava".to_string()) && none.contains(&"lazydocker".to_string()));
        assert!(!none.contains(&"yazi".to_string()), "core vem do bootstrap, não do --extras");
        let some = plan_install(&c, &[], true, &|t| t.id == "visualizer").unwrap();
        assert!(!some.contains(&"cava".to_string()));
    }

    #[test]
    fn the_clios_entry_is_never_a_package() {
        let p = plan_install(&builtin(), &["update".into()], false, &|_| false).unwrap();
        assert!(p.is_empty());
    }
}
