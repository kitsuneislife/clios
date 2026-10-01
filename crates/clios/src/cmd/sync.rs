//! `clios sync`: instala os dotfiles e renderiza o tema.

use anyhow::Result;
use clios_core::sync::{LinkAction, LinkMode, SeedAction, link_static, seed_files};

use super::theme::{self, ApplyOptions};
use crate::ctx::Ctx;
use crate::ui;

pub struct Options {
    pub copy: bool,
    pub dry_run: bool,
    pub no_theme: bool,
}

pub fn run(ctx: &Ctx, opts: Options) -> Result<()> {
    let mode = if opts.copy { LinkMode::Copy } else { LinkMode::Symlink };
    let linked = link_static(&ctx.paths, mode, opts.dry_run)?;

    let (mut new, mut ok, mut saved) = (0, 0, 0);
    for l in &linked {
        let rel = l.dst.strip_prefix(&ctx.paths.config).unwrap_or(&l.dst).display();
        match &l.action {
            LinkAction::AlreadyOk => ok += 1,
            LinkAction::Linked | LinkAction::Copied => {
                new += 1;
                println!("{} {rel}", ui::dim("novo   "));
            }
            LinkAction::BackedUp(bak) => {
                saved += 1;
                println!("{} {rel}  {}", ui::dim("backup "), ui::dim(&format!("(o seu foi para {})", bak.display())));
            }
        }
    }
    println!(
        "{}",
        ui::dim(&format!(
            "{new} ligados, {saved} com backup, {ok} já certos{}",
            if opts.dry_run { " (simulação)" } else { "" }
        ))
    );

    for s in seed_files(&ctx.paths, opts.dry_run)? {
        if s.action == SeedAction::Seeded {
            let rel = s.dst.strip_prefix(&ctx.paths.config).unwrap_or(&s.dst).display();
            println!("{} {rel}", ui::dim("semente"));
        }
    }

    if !opts.no_theme {
        let s = theme::apply(ctx, ApplyOptions { live: !opts.dry_run, hooks: !opts.dry_run, dry_run: opts.dry_run })?;
        theme::print_summary(ctx, &s)?;
    }
    Ok(())
}
