//! `clios sync`: renderiza os templates e liga os dotfiles estáticos.

use std::fs;
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde::Deserialize;

use crate::fsutil::write_atomic;
use crate::paths::Paths;
use crate::template;
use crate::theme::Theme;

#[derive(Debug, Deserialize)]
pub struct Manifest {
    #[serde(default)]
    pub template: Vec<TemplateEntry>,
    #[serde(default)]
    pub hook: Vec<Hook>,
}

#[derive(Debug, Deserialize)]
pub struct TemplateEntry {
    /// Relativo a `templates/`.
    pub src: String,
    /// Relativo a ~/.config, ou `@state/...`.
    pub dst: String,
}

/// Comando a rodar depois que os arquivos mudam, para o app enxergar o tema novo.
#[derive(Debug, Deserialize)]
pub struct Hook {
    pub name: String,
    /// Só roda se existir um processo com esse nome.
    pub if_running: Option<String>,
    pub run: Vec<String>,
}

impl Manifest {
    pub fn load(paths: &Paths) -> Result<Self> {
        let file = paths.templates_dir().join("manifest.toml");
        let src = fs::read_to_string(&file).with_context(|| format!("lendo {}", file.display()))?;
        toml::from_str(&src).with_context(|| format!("{} inválido", file.display()))
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum Change {
    Created,
    Updated,
    Unchanged,
}

#[derive(Debug)]
pub struct Rendered {
    pub dst: PathBuf,
    pub change: Change,
}

/// Renderiza todos os templates do manifesto. Só grava o que mudou, para não acordar
/// observadores de arquivo (Quickshell, Hyprland) sem necessidade.
pub fn render_templates(paths: &Paths, manifest: &Manifest, theme: &Theme, dry_run: bool) -> Result<Vec<Rendered>> {
    let mut out = Vec::with_capacity(manifest.template.len());
    for entry in &manifest.template {
        let src_path = paths.templates_dir().join(&entry.src);
        let source = fs::read_to_string(&src_path).with_context(|| format!("lendo template {}", src_path.display()))?;
        let rendered = template::render(&entry.src, &source, theme)?;
        let dst = paths.resolve_dst(&entry.dst);

        let change = match fs::read(&dst) {
            Ok(existing) if existing == rendered.as_bytes() => Change::Unchanged,
            Ok(_) => Change::Updated,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Change::Created,
            Err(e) => return Err(e).with_context(|| format!("lendo {}", dst.display())),
        };
        if change != Change::Unchanged && !dry_run {
            write_atomic(&dst, rendered.as_bytes())?;
        }
        out.push(Rendered { dst, change });
    }
    Ok(out)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkMode {
    /// Symlink para o checkout: editar o repo muda o sistema na hora.
    Symlink,
    /// Cópia: para montar a ISO, onde o checkout não existirá.
    Copy,
}

#[derive(Debug, PartialEq, Eq)]
pub enum LinkAction {
    Linked,
    Copied,
    AlreadyOk,
    /// Havia outro arquivo no lugar; foi movido para `.clios-bak`.
    BackedUp(PathBuf),
}

#[derive(Debug)]
pub struct Linked {
    pub dst: PathBuf,
    pub action: LinkAction,
}

pub fn link_static(paths: &Paths, mode: LinkMode, dry_run: bool) -> Result<Vec<Linked>> {
    let base = paths.static_dir();
    let mut files = Vec::new();
    collect_files(&base, &mut files)?;
    files.sort();

    let mut out = Vec::with_capacity(files.len());
    for src in files {
        let rel = src.strip_prefix(&base).expect("arquivo está dentro de config/");
        let dst = paths.config.join(rel);
        out.push(Linked { action: place(&src, &dst, mode, dry_run)?, dst });
    }
    Ok(out)
}

#[derive(Debug, PartialEq, Eq)]
pub enum SeedAction {
    Seeded,
    /// Já existe (o do usuário, ou o que o app reescreveu): intocado.
    Kept,
}

#[derive(Debug)]
pub struct Seeded {
    pub dst: PathBuf,
    pub action: SeedAction,
}

/// Copia `seed/` para ~/.config só onde ainda não existe nada.
pub fn seed_files(paths: &Paths, dry_run: bool) -> Result<Vec<Seeded>> {
    let base = paths.seed_dir();
    let mut files = Vec::new();
    collect_files(&base, &mut files)?;
    files.sort();
    let mut out = Vec::with_capacity(files.len());
    for src in files {
        let rel = src.strip_prefix(&base).expect("arquivo está dentro de seed/");
        let dst = paths.config.join(rel);
        let action = if dst.symlink_metadata().is_ok() {
            SeedAction::Kept
        } else {
            if !dry_run {
                if let Some(parent) = dst.parent() {
                    fs::create_dir_all(parent)?;
                }
                fs::copy(&src, &dst).with_context(|| format!("copiando para {}", dst.display()))?;
            }
            SeedAction::Seeded
        };
        out.push(Seeded { dst, action });
    }
    Ok(out)
}

fn collect_files(dir: &Path, out: &mut Vec<PathBuf>) -> Result<()> {
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(e).with_context(|| format!("lendo {}", dir.display())),
    };
    for entry in entries {
        let path = entry?.path();
        if path.is_dir() {
            collect_files(&path, out)?;
        } else {
            out.push(path);
        }
    }
    Ok(())
}

fn place(src: &Path, dst: &Path, mode: LinkMode, dry_run: bool) -> Result<LinkAction> {
    // Já está certo?
    match mode {
        LinkMode::Symlink => {
            if fs::read_link(dst).is_ok_and(|t| t == src) {
                return Ok(LinkAction::AlreadyOk);
            }
        }
        LinkMode::Copy => {
            if dst.is_file() && fs::read(dst)? == fs::read(src)? {
                return Ok(LinkAction::AlreadyOk);
            }
        }
    }

    let mut backed_up = None;
    if dst.symlink_metadata().is_ok() {
        if dst.is_dir() && dst.symlink_metadata()?.is_dir() {
            bail!("{} é um diretório; esperava um arquivo", dst.display());
        }
        let bak = backup_name(dst);
        if !dry_run {
            fs::rename(dst, &bak).with_context(|| format!("movendo {} para {}", dst.display(), bak.display()))?;
        }
        backed_up = Some(bak);
    }

    if !dry_run {
        if let Some(parent) = dst.parent() {
            fs::create_dir_all(parent)?;
        }
        match mode {
            LinkMode::Symlink => {
                symlink(src, dst).with_context(|| format!("ligando {} -> {}", dst.display(), src.display()))?
            }
            LinkMode::Copy => {
                fs::copy(src, dst).with_context(|| format!("copiando para {}", dst.display()))?;
            }
        }
    }

    Ok(match backed_up {
        Some(bak) => LinkAction::BackedUp(bak),
        None if mode == LinkMode::Copy => LinkAction::Copied,
        None => LinkAction::Linked,
    })
}

fn backup_name(dst: &Path) -> PathBuf {
    let name = dst.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let mut n = 0u32;
    loop {
        let suffix = if n == 0 { ".clios-bak".to_string() } else { format!(".clios-bak{n}") };
        let candidate = dst.with_file_name(format!("{name}{suffix}"));
        if candidate.symlink_metadata().is_err() {
            return candidate;
        }
        n += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tokens::{Mode, MotionLevel, Tokens};

    struct Sandbox {
        paths: Paths,
    }

    fn sandbox(name: &str) -> Sandbox {
        let base = std::env::temp_dir().join(format!("clios-sync-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        let root = base.join("root");
        fs::create_dir_all(root.join("tokens")).unwrap();
        fs::create_dir_all(root.join("templates/app")).unwrap();
        fs::create_dir_all(root.join("config/app")).unwrap();
        fs::write(root.join("tokens/tokens.toml"), "").unwrap();
        fs::write(
            root.join("templates/manifest.toml"),
            "[[template]]\nsrc = \"app/a.conf.j2\"\ndst = \"app/a.conf\"\n\n[[template]]\nsrc = \"app/s.json.j2\"\ndst = \"@state/s.json\"\n",
        )
        .unwrap();
        fs::write(root.join("templates/app/a.conf.j2"), "accent={{ c.accent | hex }}\n").unwrap();
        fs::write(root.join("templates/app/s.json.j2"), "{\"m\": \"{{ mode }}\"}\n").unwrap();
        fs::write(root.join("config/app/static.conf"), "fixo\n").unwrap();
        let home = base.join("home");
        Sandbox { paths: Paths::discover(Some(&root), Some(&home)).unwrap() }
    }

    fn theme() -> Theme {
        Theme::resolve(&Tokens::builtin(), Mode::Dark, "ember", MotionLevel::Full).unwrap()
    }

    #[test]
    fn renders_then_is_idempotent() {
        let sb = sandbox("render");
        let m = Manifest::load(&sb.paths).unwrap();
        let first = render_templates(&sb.paths, &m, &theme(), false).unwrap();
        assert!(first.iter().all(|r| r.change == Change::Created));
        assert_eq!(fs::read_to_string(sb.paths.config.join("app/a.conf")).unwrap(), "accent=#FF5A1F\n");
        assert_eq!(fs::read_to_string(sb.paths.state.join("s.json")).unwrap(), "{\"m\": \"dark\"}\n");

        let second = render_templates(&sb.paths, &m, &theme(), false).unwrap();
        assert!(second.iter().all(|r| r.change == Change::Unchanged));
    }

    #[test]
    fn dry_run_writes_nothing() {
        let sb = sandbox("dry");
        let m = Manifest::load(&sb.paths).unwrap();
        render_templates(&sb.paths, &m, &theme(), true).unwrap();
        assert!(!sb.paths.config.join("app/a.conf").exists());
    }

    #[test]
    fn links_static_files_and_is_idempotent() {
        let sb = sandbox("link");
        let first = link_static(&sb.paths, LinkMode::Symlink, false).unwrap();
        assert_eq!(first[0].action, LinkAction::Linked);
        let dst = sb.paths.config.join("app/static.conf");
        assert_eq!(fs::read_link(&dst).unwrap(), sb.paths.root.join("config/app/static.conf"));
        let second = link_static(&sb.paths, LinkMode::Symlink, false).unwrap();
        assert_eq!(second[0].action, LinkAction::AlreadyOk);
    }

    #[test]
    fn existing_user_file_is_backed_up_not_destroyed() {
        let sb = sandbox("bak");
        let dst = sb.paths.config.join("app/static.conf");
        fs::create_dir_all(dst.parent().unwrap()).unwrap();
        fs::write(&dst, "do usuário\n").unwrap();
        let out = link_static(&sb.paths, LinkMode::Symlink, false).unwrap();
        assert!(matches!(&out[0].action, LinkAction::BackedUp(_)));
        assert_eq!(fs::read_to_string(dst.with_file_name("static.conf.clios-bak")).unwrap(), "do usuário\n");
        assert_eq!(fs::read_to_string(&dst).unwrap(), "fixo\n");
    }

    #[test]
    fn copy_mode_makes_real_files() {
        let sb = sandbox("copy");
        link_static(&sb.paths, LinkMode::Copy, false).unwrap();
        let dst = sb.paths.config.join("app/static.conf");
        assert!(!dst.symlink_metadata().unwrap().file_type().is_symlink());
        assert_eq!(fs::read_to_string(dst).unwrap(), "fixo\n");
    }

    #[test]
    fn seed_copies_once_and_never_overwrites() {
        let sb = sandbox("seed");
        fs::create_dir_all(sb.paths.root.join("seed/app")).unwrap();
        fs::write(sb.paths.root.join("seed/app/app.conf"), "padrão\n").unwrap();
        let first = seed_files(&sb.paths, false).unwrap();
        assert_eq!(first[0].action, SeedAction::Seeded);
        let dst = sb.paths.config.join("app/app.conf");
        assert!(!dst.symlink_metadata().unwrap().file_type().is_symlink(), "seed é cópia, não link");
        // o app reescreve o arquivo...
        fs::write(&dst, "do app\n").unwrap();
        let second = seed_files(&sb.paths, false).unwrap();
        assert_eq!(second[0].action, SeedAction::Kept);
        assert_eq!(fs::read_to_string(&dst).unwrap(), "do app\n", "a escolha do app/usuário sobrevive");
    }

    #[test]
    fn seed_dry_run_writes_nothing() {
        let sb = sandbox("seeddry");
        fs::create_dir_all(sb.paths.root.join("seed")).unwrap();
        fs::write(sb.paths.root.join("seed/x.conf"), "x").unwrap();
        seed_files(&sb.paths, true).unwrap();
        assert!(!sb.paths.config.join("x.conf").exists());
    }

    #[test]
    fn dry_run_link_touches_nothing() {
        let sb = sandbox("drylink");
        link_static(&sb.paths, LinkMode::Symlink, true).unwrap();
        assert!(!sb.paths.config.join("app").exists());
    }
}
