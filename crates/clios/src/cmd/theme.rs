//! `clios theme`: escolher modo e acento, e espalhar o resultado.

use std::fs;
use std::io::Write;
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::process::{Command, Stdio};

use anyhow::Result;
use clios_core::sync::{Change, Manifest, render_templates};
use clios_core::terminal::{kernel_vt_params, recolor_sequence};
use clios_core::{Mode, MotionLevel, Theme};

use crate::ctx::Ctx;
use crate::ui;

#[derive(Debug, Clone, Copy)]
pub struct ApplyOptions {
    /// Recolore os terminais abertos.
    pub live: bool,
    /// Avisa Hyprland, helix e btop.
    pub hooks: bool,
    pub dry_run: bool,
}

impl Default for ApplyOptions {
    fn default() -> Self {
        Self { live: true, hooks: true, dry_run: false }
    }
}

pub struct Summary {
    pub changed: usize,
    pub unchanged: usize,
    pub terminals: usize,
}

pub fn apply(ctx: &Ctx, mut opts: ApplyOptions) -> Result<Summary> {
    if ctx.sandboxed {
        // Árvore simulada: gravar arquivos sim, mexer no sistema vivo jamais.
        opts.live = false;
        opts.hooks = false;
    }
    let theme = ctx.theme()?;
    let manifest = Manifest::load(&ctx.paths)?;
    let rendered = render_templates(&ctx.paths, &manifest, &theme, opts.dry_run)?;

    let changed = rendered.iter().filter(|r| r.change != Change::Unchanged).count();
    let unchanged = rendered.len() - changed;

    if opts.dry_run {
        for r in rendered.iter().filter(|r| r.change != Change::Unchanged) {
            println!("{} {}", ui::dim("mudaria"), r.dst.display());
        }
        return Ok(Summary { changed, unchanged, terminals: 0 });
    }

    if opts.hooks && changed > 0 {
        run_hooks(&manifest);
    }
    let terminals = if opts.live { recolor_terminals(&theme) } else { 0 };
    Ok(Summary { changed, unchanged, terminals })
}

fn run_hooks(manifest: &Manifest) {
    for hook in &manifest.hook {
        if let Some(proc_name) = &hook.if_running {
            let running = Command::new("pgrep")
                .args(["-x", proc_name])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .is_ok_and(|s| s.success());
            if !running {
                continue;
            }
        }
        let Some((prog, args)) = hook.run.split_first() else { continue };
        let ok = Command::new(prog)
            .args(args)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok_and(|s| s.success());
        if !ok {
            // Hook nunca derruba o apply: o tema já está gravado.
            eprintln!("aviso: hook {:?} falhou (o tema foi aplicado mesmo assim)", hook.name);
        }
    }
}

/// Escreve a sequência OSC em cada pty do usuário. Devolve quantos aceitaram.
fn recolor_terminals(theme: &Theme) -> usize {
    // O dono de /proc/self é o uid do processo; evita puxar libc só para isso.
    let Ok(me) = fs::metadata("/proc/self").map(|m| m.uid()) else { return 0 };
    let Ok(dir) = fs::read_dir("/dev/pts") else { return 0 };
    let seq = recolor_sequence(theme);

    // Valores de O_NOCTTY | O_NONBLOCK no Linux: não podemos nos prender a um pty parado.
    const O_NOCTTY_NONBLOCK: i32 = 0o400 | 0o4000;

    let mut count = 0;
    for entry in dir.flatten() {
        let name = entry.file_name();
        if !name.to_string_lossy().chars().all(|c| c.is_ascii_digit()) {
            continue;
        }
        let path = entry.path();
        if fs::metadata(&path).map(|m| m.uid()).ok() != Some(me) {
            continue;
        }
        let Ok(mut f) = fs::OpenOptions::new().write(true).custom_flags(O_NOCTTY_NONBLOCK).open(&path) else {
            continue;
        };
        if f.write_all(seq.as_bytes()).is_ok() {
            count += 1;
        }
    }
    count
}

pub fn print_summary(ctx: &Ctx, s: &Summary) -> Result<()> {
    let theme = ctx.theme()?;
    let mut line = format!("{} {} · {}", ui::swatch(theme.c.accent), theme.mode, theme.accent_name);
    if theme.motion.level != MotionLevel::Full {
        line.push_str(&format!(" · movimento {}", theme.motion.level));
    }
    line.push_str(&ui::dim(&format!(
        "   {} atualizados, {} iguais{}",
        s.changed,
        s.unchanged,
        if s.terminals > 0 { format!(", {} terminais", s.terminals) } else { String::new() }
    )));
    println!("{line}");
    Ok(())
}

/// Salva a escolha, valida o resultado antes de gravar, e aplica.
pub fn set(ctx: &mut Ctx, mode: Option<Mode>, accent: Option<String>, motion: Option<MotionLevel>) -> Result<()> {
    let mut next = ctx.state.clone();
    if let Some(m) = mode {
        next.mode = m;
    }
    if let Some(a) = accent {
        next.accent = a;
    }
    if let Some(m) = motion {
        next.motion = m;
    }
    // Resolver antes de gravar: acento inválido não pode corromper o estado salvo.
    ctx.theme_for(&next)?;
    ctx.state = next;
    ctx.save_state()?;
    let s = apply(ctx, ApplyOptions::default())?;
    print_summary(ctx, &s)
}

pub fn cycle_accent(ctx: &mut Ctx) -> Result<()> {
    let names: Vec<String> = ctx.theme()?.accents.iter().map(|a| a.name.clone()).collect();
    let current = names.iter().position(|n| *n == ctx.state.accent).unwrap_or(names.len() - 1);
    let next = names[(current + 1) % names.len()].clone();
    set(ctx, None, Some(next), None)
}

pub fn list(ctx: &Ctx) -> Result<()> {
    let theme = ctx.theme()?;
    for a in &theme.accents {
        let here = if a.name == theme.accent_name { "  ←" } else { "" };
        println!("{} {:<8} {}{here}", ui::swatch(a.color), a.name, a.color.hex());
    }
    if theme.accent_name == "custom" {
        println!("{} {:<8} {}  ←", ui::swatch(theme.c.accent), "custom", theme.c.accent.hex());
    }
    Ok(())
}

pub fn show(ctx: &Ctx) -> Result<()> {
    let t = ctx.theme()?;
    println!("{}", ui::bold(&format!("{} · {} · movimento {}", t.mode, t.accent_name, t.motion.level)));
    let rows = [
        ("bg", t.c.bg),
        ("surface", t.c.surface),
        ("raised", t.c.raised),
        ("line", t.c.line),
        ("mute", t.c.mute),
        ("dim", t.c.dim),
        ("fg", t.c.fg),
        ("accent", t.c.accent),
        ("accent_dim", t.c.accent_dim),
        ("accent_soft", t.c.accent_soft),
        ("on_accent", t.c.on_accent),
    ];
    for (name, c) in rows {
        println!("{} {:<12} {}  {}", ui::swatch(c), name, c.hex(), ui::dim(&format!("{:.1}:1", c.contrast(t.c.bg))));
    }
    let ansi: String = t.c.ansi.iter().map(|c| ui::swatch(*c)).collect::<Vec<_>>().join("");
    println!("{ansi}");
    Ok(())
}

pub fn cmdline(ctx: &Ctx) -> Result<()> {
    // O console de texto é sempre escuro, independente do modo escolhido para o desktop.
    let dark = Theme::resolve(&ctx.tokens, Mode::Dark, &ctx.state.accent, ctx.state.motion)?;
    println!("{}", kernel_vt_params(&dark));
    Ok(())
}

pub fn toggle(ctx: &mut Ctx) -> Result<()> {
    let next = ctx.state.mode.toggled();
    set(ctx, Some(next), None, None)
}
