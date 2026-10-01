//! `clios prompt`: o estilo do prompt do shell. O starship relê a config a cada prompt,
//! então a troca vale na hora, até nos terminais que já estão abertos.

use anyhow::Result;
use clios_core::PromptStyle;

use super::theme::{self, ApplyOptions};
use crate::ctx::Ctx;
use crate::ui;

pub fn run(ctx: &mut Ctx, style: Option<PromptStyle>) -> Result<()> {
    match style {
        None => {
            for p in PromptStyle::ALL {
                let here = if p == ctx.state.prompt { "  ←" } else { "" };
                println!("{:<8} {}{here}", p.id(), ui::dim(p.blurb()));
            }
            Ok(())
        }
        Some(p) => {
            set_quiet(ctx, p)?;
            println!("prompt {} · {}", p.id(), ui::dim(p.blurb()));
            Ok(())
        }
    }
}

/// Grava a escolha e regera a config do starship (sem tocar em terminais nem em hooks: nada disso muda).
pub fn set_quiet(ctx: &mut Ctx, style: PromptStyle) -> Result<()> {
    ctx.state.prompt = style;
    ctx.save_state()?;
    theme::apply(ctx, ApplyOptions { live: false, hooks: false, dry_run: false })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use clios_core::{Mode, MotionLevel, Theme, Tokens};

    const TEMPLATE: &str = include_str!("../../../../templates/starship/starship.toml.j2");

    fn render(style: PromptStyle) -> toml::Table {
        let mut t = Theme::resolve(&Tokens::builtin(), Mode::Dark, "ember", MotionLevel::Full).unwrap();
        t.prompt = style.id().to_string();
        let text = clios_core::template::render("starship", TEMPLATE, &t).unwrap();
        text.parse::<toml::Table>()
            .unwrap_or_else(|e| panic!("{style}: o starship.toml gerado é TOML inválido: {e}\n{text}"))
    }

    fn s<'a>(t: &'a toml::Table, k: &str) -> &'a str {
        t[k].as_str().unwrap_or_else(|| panic!("falta {k}"))
    }

    #[test]
    fn every_style_renders_valid_toml_with_the_clios_palette() {
        for p in PromptStyle::ALL {
            let t = render(p);
            assert_eq!(s(&t, "palette"), "clios", "{p}");
            let accent = t["palettes"]["clios"]["accent"].as_str().unwrap();
            assert_eq!(accent, "#FF5A1F", "{p}: o acento vem do tema");
        }
    }

    #[test]
    fn the_styles_differ_where_they_should() {
        let (min, dev, zen) = (render(PromptStyle::Minimal), render(PromptStyle::Dev), render(PromptStyle::Zen));
        assert!(!s(&min, "format").contains('\n'), "minimal é uma linha só");
        assert!(s(&min, "format").ends_with("$character"));
        assert!(s(&dev, "format").contains('\n'), "dev tem duas linhas");
        assert!(s(&dev, "format").ends_with("\n$character"), "e a seta fica sozinha embaixo");
        assert!(s(&dev, "format").contains("$nodejs") && s(&dev, "format").contains("$rust"));
        assert_eq!(s(&zen, "format"), "$character", "zen: só a seta");
        assert!(s(&zen, "right_format").contains("$directory"), "zen: a pasta vai para a direita");
        assert!(!s(&min, "format").contains("$nodejs"), "as linguagens só aparecem no dev");
    }

    #[test]
    fn switching_the_style_rewrites_the_starship_config() {
        let d = std::env::temp_dir().join(format!("clios-prompt-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let mut ctx = Ctx::load(Some(&root), Some(&d)).unwrap();
        set_quiet(&mut ctx, PromptStyle::Zen).unwrap();
        let out = std::fs::read_to_string(d.join(".config/starship.toml")).unwrap();
        assert!(out.contains("format = \"$character\""), "{out}");
        assert_eq!(ctx.state.prompt, PromptStyle::Zen);
        set_quiet(&mut ctx, PromptStyle::Dev).unwrap();
        assert!(std::fs::read_to_string(d.join(".config/starship.toml")).unwrap().contains("$nodejs"));
        let _ = std::fs::remove_dir_all(&d);
    }
}
