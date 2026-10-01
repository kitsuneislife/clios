//! `clios wallpaper`: papéis de parede procedurais (tema) e imagens do usuário.
//!
//! O estado guarda só a escolha (`grade`, `file:praia.jpg`). `apply` transforma a escolha em um arquivo
//! em `~/.local/state/clios/wallpaper/` e avisa a shell por `wallpaper.json`, que ela observa e cruza o fade.

use std::fs;
use std::io::IsTerminal;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, bail};
use clios_core::fsutil::write_atomic;
use clios_core::wallpaper::{self, Style};
use clios_core::{Paths, Theme};
use serde::{Deserialize, Serialize};

use crate::ctx::Ctx;
use crate::ui;

const IMAGE_EXTS: [&str; 5] = ["png", "jpg", "jpeg", "webp", "avif"];
/// Muda quando o desenho dos estilos muda, para invalidar o que já foi renderizado.
const RENDER_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Choice {
    Style(Style),
    File(String),
}

impl Choice {
    pub fn parse(id: &str) -> Result<Self> {
        match id.strip_prefix("file:") {
            Some(name) => Ok(Choice::File(name.to_string())),
            None => Ok(Choice::Style(id.parse()?)),
        }
    }

    pub fn id(&self) -> String {
        match self {
            Choice::Style(s) => s.id().to_string(),
            Choice::File(n) => format!("file:{n}"),
        }
    }

    pub fn label(&self) -> String {
        match self {
            Choice::Style(s) => s.name().to_string(),
            Choice::File(n) => n.clone(),
        }
    }
}

fn is_image(p: &Path) -> bool {
    p.extension().and_then(|e| e.to_str()).is_some_and(|e| IMAGE_EXTS.contains(&e.to_ascii_lowercase().as_str()))
}

/// Imagens que o usuário colocou em `~/.local/share/clios/wallpapers`, em ordem alfabética.
pub fn user_files(paths: &Paths) -> Vec<String> {
    let Ok(dir) = fs::read_dir(paths.wallpapers_dir()) else { return Vec::new() };
    let mut names: Vec<String> =
        dir.flatten().filter(|e| is_image(&e.path())).filter_map(|e| e.file_name().into_string().ok()).collect();
    names.sort_by_key(|n| n.to_lowercase());
    names
}

/// Os estilos do tema e depois as imagens do usuário.
pub fn all_choices(paths: &Paths) -> Vec<Choice> {
    Style::ALL.into_iter().map(Choice::Style).chain(user_files(paths).into_iter().map(Choice::File)).collect()
}

/// O vizinho de `current` na lista, dando a volta nas pontas.
pub fn step(all: &[Choice], current: &Choice, delta: isize) -> Choice {
    let i = all.iter().position(|c| c == current).unwrap_or(0) as isize;
    all[(i + delta).rem_euclid(all.len() as isize) as usize].clone()
}

/// Interpreta o que o usuário digitou: um estilo, o nome de uma imagem dele, ou o caminho de uma imagem nova.
pub fn resolve(ctx: &Ctx, what: &str) -> Result<Choice> {
    if let Ok(c) = Choice::parse(what) {
        if matches!(&c, Choice::Style(_)) || matches!(&c, Choice::File(n) if user_files(&ctx.paths).contains(n)) {
            return Ok(c);
        }
    }
    if user_files(&ctx.paths).iter().any(|n| n == what) {
        return Ok(Choice::File(what.to_string()));
    }
    if Path::new(what).is_file() {
        return Ok(Choice::File(add_file(&ctx.paths, Path::new(what))?));
    }
    bail!("não achei o papel de parede {what:?}. Veja as opções com: clios wallpaper list")
}

/// Copia uma imagem para a pasta do usuário. Nome repetido ganha sufixo, nunca sobrescreve.
pub fn add_file(paths: &Paths, src: &Path) -> Result<String> {
    if !is_image(src) {
        bail!("{} não parece uma imagem (aceito: {})", src.display(), IMAGE_EXTS.join(", "));
    }
    let dir = paths.wallpapers_dir();
    fs::create_dir_all(&dir)?;
    let stem = src.file_stem().and_then(|s| s.to_str()).unwrap_or("imagem");
    let ext = src.extension().and_then(|s| s.to_str()).unwrap_or("png").to_ascii_lowercase();
    let mut name = format!("{stem}.{ext}");
    let mut n = 2;
    while dir.join(&name).exists() {
        if fs::read(dir.join(&name)).ok() == fs::read(src).ok() {
            return Ok(name); // já é a mesma imagem
        }
        name = format!("{stem}-{n}.{ext}");
        n += 1;
    }
    fs::copy(src, dir.join(&name)).with_context(|| format!("copiando {}", src.display()))?;
    Ok(name)
}

// ── tamanho da tela ────────────────────────────────────────────────────────

/// O maior monitor do `hyprctl monitors -j`, limitado a 4K (acima disso só gasta disco).
pub fn parse_monitors(json: &str) -> Option<(u32, u32)> {
    let v: serde_json::Value = serde_json::from_str(json).ok()?;
    let (w, h) = v
        .as_array()?
        .iter()
        .filter_map(|m| Some((m.get("width")?.as_u64()? as u32, m.get("height")?.as_u64()? as u32)))
        .max_by_key(|(w, h)| w * h)?;
    Some(clamp_size(w, h))
}

fn clamp_size(w: u32, h: u32) -> (u32, u32) {
    let k = (3840.0 / f64::from(w)).min(2160.0 / f64::from(h)).min(1.0);
    (((f64::from(w) * k).round() as u32).max(640), ((f64::from(h) * k).round() as u32).max(360))
}

fn target_size(ctx: &Ctx) -> (u32, u32) {
    if let Some(s) = std::env::var("CLIOS_WALLPAPER_SIZE").ok().and_then(|s| {
        let (w, h) = s.split_once('x')?;
        Some((w.parse().ok()?, h.parse().ok()?))
    }) {
        return s;
    }
    if ctx.sandboxed {
        return (960, 540);
    }
    Command::new("hyprctl")
        .args(["monitors", "-j"])
        .output()
        .ok()
        .and_then(|o| parse_monitors(&String::from_utf8_lossy(&o.stdout)))
        .unwrap_or((2560, 1440))
}

// ── aplicar ────────────────────────────────────────────────────────────────

/// O que a shell lê para saber qual imagem mostrar.
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Pointer {
    pub path: String,
    pub key: String,
    pub rev: u64,
    pub id: String,
}

fn dir(paths: &Paths) -> PathBuf {
    paths.state.join("wallpaper")
}

fn pointer_file(paths: &Paths) -> PathBuf {
    paths.state.join("wallpaper.json")
}

fn load_pointer(paths: &Paths) -> Option<Pointer> {
    serde_json::from_str(&fs::read_to_string(pointer_file(paths)).ok()?).ok()
}

fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_millis() as u64)
}

/// A chave diz se o que já está no disco ainda vale: mesmo estilo, modo, acento e tamanho.
fn style_key(style: Style, theme: &Theme, size: (u32, u32)) -> String {
    format!("{}|{}|{}|{}x{}|v{RENDER_VERSION}", style.id(), theme.mode, theme.c.accent.hex(), size.0, size.1)
}

#[derive(Debug, PartialEq, Eq)]
pub enum Applied {
    Rendered,
    Unchanged,
}

/// Faz a escolha do estado virar um arquivo e avisa a shell. Barato quando nada mudou.
pub fn apply(ctx: &Ctx) -> Result<Applied> {
    let theme = ctx.theme()?;
    let choice = Choice::parse(&ctx.state.wallpaper).unwrap_or(Choice::Style(Style::Grade));
    let old = load_pointer(&ctx.paths);
    let out_dir = dir(&ctx.paths);

    let (key, source): (String, Source) = match &choice {
        Choice::Style(s) => (style_key(*s, &theme, target_size(ctx)), Source::Render(*s)),
        Choice::File(name) => {
            let path = ctx.paths.wallpapers_dir().join(name);
            if !path.is_file() {
                // A imagem sumiu: volta para o padrão em vez de deixar a tela sem fundo.
                eprintln!("aviso: {} não existe mais; usando o estilo padrão", path.display());
                let s = Style::Grade;
                (style_key(s, &theme, target_size(ctx)), Source::Render(s))
            } else {
                let mtime =
                    fs::metadata(&path).and_then(|m| m.modified()).ok().and_then(|t| t.duration_since(UNIX_EPOCH).ok());
                (format!("file|{name}|{}", mtime.map_or(0, |d| d.as_secs())), Source::File(path))
            }
        }
    };

    let current = out_dir.join("current.png");
    if old.as_ref().is_some_and(|p| p.key == key && Path::new(&p.path).exists()) && current.exists() {
        return Ok(Applied::Unchanged);
    }

    let rev = now_ms();
    let target = match source {
        Source::Render(style) => {
            let (w, h) = target_size(ctx);
            let png = wallpaper::encode_png(w, h, &wallpaper::render(style, &theme.c, w, h))?;
            let path = out_dir.join(format!("wp-{rev}.png"));
            write_atomic(&path, &png)?;
            path
        }
        Source::File(path) => path,
    };

    // `current.png` é o caminho estável (hyprlock); o arquivo com `rev` no nome é o que a shell recarrega.
    fs::create_dir_all(&out_dir)?;
    let tmp = out_dir.join(".current.tmp");
    let _ = fs::remove_file(&tmp);
    std::os::unix::fs::symlink(&target, &tmp)?;
    fs::rename(&tmp, &current)?;

    let pointer = Pointer { path: target.display().to_string(), key, rev, id: choice.id() };
    write_atomic(&pointer_file(&ctx.paths), serde_json::to_string_pretty(&pointer)?.as_bytes())?;
    prune(&out_dir, &target);
    Ok(Applied::Rendered)
}

enum Source {
    Render(Style),
    File(PathBuf),
}

/// Mantém só o arquivo atual e o anterior (a shell ainda pode estar cruzando o fade com ele).
fn prune(dir: &Path, keep: &Path) {
    let Ok(rd) = fs::read_dir(dir) else { return };
    let mut old: Vec<PathBuf> = rd
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.starts_with("wp-") && n.ends_with(".png")))
        .filter(|p| p != keep)
        .collect();
    old.sort();
    let drop = old.len().saturating_sub(1);
    for p in old.into_iter().take(drop) {
        let _ = fs::remove_file(p);
    }
}

// ── comandos ───────────────────────────────────────────────────────────────

fn set_choice(ctx: &mut Ctx, choice: &Choice) -> Result<()> {
    ctx.state.wallpaper = choice.id();
    ctx.save_state()?;
    apply(ctx)?;
    Ok(())
}

pub fn set(ctx: &mut Ctx, what: &str) -> Result<()> {
    let choice = resolve(ctx, what)?;
    set_choice(ctx, &choice)?;
    println!("papel de parede: {}", choice.label());
    Ok(())
}

/// Passa para o vizinho na lista, sem imprimir.
pub fn step_quiet(ctx: &mut Ctx, delta: isize) -> Result<Choice> {
    let all = all_choices(&ctx.paths);
    let cur = Choice::parse(&ctx.state.wallpaper).unwrap_or(Choice::Style(Style::Grade));
    let next = step(&all, &cur, delta);
    set_choice(ctx, &next)?;
    Ok(next)
}

pub fn step_cmd(ctx: &mut Ctx, delta: isize) -> Result<()> {
    println!("papel de parede: {}", step_quiet(ctx, delta)?.label());
    Ok(())
}

pub fn random(ctx: &mut Ctx) -> Result<()> {
    let all = all_choices(&ctx.paths);
    let cur = Choice::parse(&ctx.state.wallpaper).unwrap_or(Choice::Style(Style::Grade));
    let others: Vec<&Choice> = all.iter().filter(|c| **c != cur).collect();
    if others.is_empty() {
        return Ok(());
    }
    let pick = others[(now_ms() as usize / 7) % others.len()].clone();
    set_choice(ctx, &pick)?;
    println!("papel de parede: {}", pick.label());
    Ok(())
}

pub fn list(ctx: &Ctx) -> Result<()> {
    let current = ctx.state.wallpaper.clone();
    for c in all_choices(&ctx.paths) {
        let mark = if c.id() == current { "●" } else { " " };
        let desc = match &c {
            Choice::Style(s) => s.desc().to_string(),
            Choice::File(_) => "sua imagem".to_string(),
        };
        println!("{mark} {:<14} {}", c.id(), ui::dim(&desc));
    }
    Ok(())
}

pub fn path(ctx: &Ctx) -> Result<()> {
    apply(ctx)?;
    println!("{}", dir(&ctx.paths).join("current.png").display());
    Ok(())
}

pub fn add(ctx: &mut Ctx, file: &Path, set_it: bool) -> Result<()> {
    let name = add_file(&ctx.paths, file)?;
    println!("adicionado: {name}");
    if set_it {
        set_choice(ctx, &Choice::File(name))?;
    }
    Ok(())
}

pub fn remove(ctx: &mut Ctx, name: &str) -> Result<()> {
    let name = name.strip_prefix("file:").unwrap_or(name);
    let path = ctx.paths.wallpapers_dir().join(name);
    if !path.is_file() {
        bail!("não há a imagem {name:?} em {}", ctx.paths.wallpapers_dir().display());
    }
    fs::remove_file(&path)?;
    if ctx.state.wallpaper == format!("file:{name}") {
        set_choice(ctx, &Choice::Style(Style::Grade))?;
    }
    println!("removido: {name}");
    Ok(())
}

/// Sem subcomando: o seletor interativo (ou a lista, se não houver terminal).
pub fn pick(ctx: &mut Ctx) -> Result<()> {
    if !std::io::stdout().is_terminal() {
        return list(ctx);
    }
    super::wallpaper_tui::run(ctx)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sandbox(name: &str) -> Ctx {
        let home = std::env::temp_dir().join(format!("clios-wp-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&home);
        fs::create_dir_all(&home).unwrap();
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        Ctx::load(Some(&root), Some(&home)).unwrap()
    }

    fn tiny_png(path: &Path) {
        let rgb = [200u8; 4 * 4 * 3];
        fs::write(path, wallpaper::encode_png(4, 4, &rgb).unwrap()).unwrap();
    }

    #[test]
    fn choice_ids_roundtrip() {
        for s in Style::ALL {
            assert_eq!(Choice::parse(s.id()).unwrap(), Choice::Style(s));
        }
        assert_eq!(Choice::parse("file:a b.png").unwrap(), Choice::File("a b.png".into()));
        assert!(Choice::parse("nada").is_err());
    }

    #[test]
    fn stepping_wraps_in_both_directions() {
        let all: Vec<Choice> = Style::ALL.into_iter().map(Choice::Style).collect();
        let last = all.last().unwrap().clone();
        assert_eq!(step(&all, &last, 1), all[0]);
        assert_eq!(step(&all, &all[0], -1), last);
        assert_eq!(step(&all, &Choice::File("x.png".into()), 1), all[1], "escolha desconhecida começa do início");
    }

    #[test]
    fn monitors_pick_the_biggest_and_cap_at_4k() {
        let json = r#"[{"width":1920,"height":1080},{"width":5120,"height":2880},{"width":2560,"height":1440}]"#;
        assert_eq!(parse_monitors(json), Some((3840, 2160)));
        assert_eq!(parse_monitors(r#"[{"width":1920,"height":1200}]"#), Some((1920, 1200)));
        assert_eq!(parse_monitors("[]"), None);
        assert_eq!(parse_monitors("lixo"), None);
        // ultrawide: a altura manda
        assert_eq!(parse_monitors(r#"[{"width":5120,"height":1440}]"#), Some((3840, 1080)));
    }

    #[test]
    fn apply_renders_once_then_skips_until_the_theme_changes() {
        let mut ctx = sandbox("apply");
        assert_eq!(apply(&ctx).unwrap(), Applied::Rendered);
        let current = dir(&ctx.paths).join("current.png");
        assert!(current.exists());
        let p = load_pointer(&ctx.paths).unwrap();
        assert_eq!(p.id, "grade");
        assert!(Path::new(&p.path).exists());
        assert_eq!(apply(&ctx).unwrap(), Applied::Unchanged);

        ctx.state.accent = "azure".into();
        assert_eq!(apply(&ctx).unwrap(), Applied::Rendered, "acento novo, imagem nova");
        assert_ne!(load_pointer(&ctx.paths).unwrap().rev, p.rev);

        ctx.state.mode = clios_core::Mode::Light;
        assert_eq!(apply(&ctx).unwrap(), Applied::Rendered);
    }

    #[test]
    fn old_renders_are_pruned_but_the_previous_one_stays() {
        let mut ctx = sandbox("prune");
        for s in ["grade", "aneis", "linhas", "blocos"] {
            ctx.state.wallpaper = s.into();
            apply(&ctx).unwrap();
            std::thread::sleep(std::time::Duration::from_millis(3));
        }
        let n = fs::read_dir(dir(&ctx.paths))
            .unwrap()
            .flatten()
            .filter(|e| e.file_name().to_string_lossy().starts_with("wp-"))
            .count();
        assert_eq!(n, 2, "o atual e o anterior");
    }

    #[test]
    fn user_images_are_added_listed_set_and_removed() {
        let mut ctx = sandbox("user");
        let src = ctx.paths.home.join("praia.png");
        tiny_png(&src);
        assert!(user_files(&ctx.paths).is_empty());

        add(&mut ctx, &src, true).unwrap();
        assert_eq!(user_files(&ctx.paths), ["praia.png"]);
        assert_eq!(ctx.state.wallpaper, "file:praia.png");
        let p = load_pointer(&ctx.paths).unwrap();
        assert!(p.path.ends_with("wallpapers/praia.png"), "{}", p.path);
        assert_eq!(fs::read_link(dir(&ctx.paths).join("current.png")).unwrap().to_string_lossy(), p.path);

        // resolve: pelo nome, e por um caminho novo
        assert_eq!(resolve(&ctx, "praia.png").unwrap(), Choice::File("praia.png".into()));
        assert!(resolve(&ctx, "nao-existe.png").is_err());

        remove(&mut ctx, "praia.png").unwrap();
        assert!(user_files(&ctx.paths).is_empty());
        assert_eq!(ctx.state.wallpaper, "grade", "removeu o que estava em uso: volta ao padrão");
    }

    #[test]
    fn same_name_different_image_gets_a_suffix_but_same_image_is_reused() {
        let ctx = sandbox("dupe");
        let a = ctx.paths.home.join("a/foto.png");
        fs::create_dir_all(a.parent().unwrap()).unwrap();
        tiny_png(&a);
        let b = ctx.paths.home.join("b/foto.png");
        fs::create_dir_all(b.parent().unwrap()).unwrap();
        fs::write(&b, wallpaper::encode_png(4, 4, &[10u8; 48]).unwrap()).unwrap();
        assert_eq!(add_file(&ctx.paths, &a).unwrap(), "foto.png");
        assert_eq!(add_file(&ctx.paths, &a).unwrap(), "foto.png");
        assert_eq!(add_file(&ctx.paths, &b).unwrap(), "foto-2.png");
    }

    #[test]
    fn non_images_are_refused() {
        let ctx = sandbox("txt");
        let f = ctx.paths.home.join("nota.txt");
        fs::write(&f, "oi").unwrap();
        assert!(add_file(&ctx.paths, &f).unwrap_err().to_string().contains("não parece uma imagem"));
    }

    #[test]
    fn a_vanished_user_image_falls_back_instead_of_failing() {
        let mut ctx = sandbox("vanish");
        ctx.state.wallpaper = "file:sumiu.png".into();
        assert_eq!(apply(&ctx).unwrap(), Applied::Rendered);
        assert!(load_pointer(&ctx.paths).unwrap().key.starts_with("grade|"), "caiu no estilo padrão");
        assert!(dir(&ctx.paths).join("current.png").exists());
    }
}
