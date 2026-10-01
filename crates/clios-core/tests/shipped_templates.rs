//! Os templates que vão no repositório precisam renderizar em toda combinação possível,
//! e os formatos estruturados precisam sair válidos.

use std::path::{Path, PathBuf};

use clios_core::sync::Manifest;
use clios_core::{Mode, MotionLevel, Paths, Theme, Tokens, template};

fn paths() -> Paths {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let home = std::env::temp_dir().join(format!("clios-shipped-{}", std::process::id()));
    Paths::discover(Some(&root), Some(&home)).expect("raiz do repositório")
}

fn combos() -> Vec<(Mode, String, MotionLevel)> {
    let tokens = Tokens::builtin();
    let mut accents: Vec<String> = tokens.accent.keys().cloned().collect();
    accents.push("#7CFF00".into());
    let mut out = Vec::new();
    for mode in [Mode::Dark, Mode::Light] {
        for accent in &accents {
            for level in [MotionLevel::Full, MotionLevel::Reduced, MotionLevel::Off] {
                out.push((mode, accent.clone(), level));
            }
        }
    }
    out
}

#[test]
fn every_template_renders_in_every_combination() {
    let paths = paths();
    let manifest = Manifest::load(&paths).expect("manifest.toml");
    assert!(manifest.template.len() >= 10, "esperava os templates principais");
    let tokens = Tokens::builtin();

    for (mode, accent, level) in combos() {
        let theme = Theme::resolve(&tokens, mode, &accent, level).unwrap();
        for entry in &manifest.template {
            let src_path: PathBuf = paths.templates_dir().join(&entry.src);
            let src = std::fs::read_to_string(&src_path).unwrap();
            let out = template::render(&entry.src, &src, &theme)
                .unwrap_or_else(|e| panic!("{} ({mode}, {accent}, {level}): {e:#}", entry.src));

            assert!(!out.contains("{{") && !out.contains("{%"), "{}: sobrou sintaxe de template", entry.src);
            assert!(!out.contains("None") || entry.src.contains("lazygit"), "{}: valor None vazou", entry.src);

            if entry.dst.ends_with(".json") {
                let v: serde_json::Value = serde_json::from_str(&out).unwrap_or_else(|e| {
                    panic!("{} ({mode}, {accent}, {level}) não é JSON válido: {e}\n{out}", entry.src)
                });
                // O contrato com o QML (Tokens.qml): se uma chave sumir, a shell quebra em silêncio.
                for key in ["mode", "accent", "font", "ui", "color", "motion", "accents"] {
                    assert!(v.get(key).is_some(), "theme.json sem a chave {key:?}");
                }
                for key in [
                    "bg",
                    "surface",
                    "raised",
                    "line",
                    "mute",
                    "dim",
                    "fg",
                    "accent",
                    "accentDim",
                    "accentSoft",
                    "onAccent",
                ] {
                    assert!(v["color"][key].is_string(), "color.{key} ausente");
                }
                for key in ["enabled", "spatial", "instant", "fast", "base", "slow", "curve"] {
                    assert!(v["motion"].get(key).is_some(), "motion.{key} ausente");
                }
                assert_eq!(v["motion"]["curve"]["out"].as_array().map(Vec::len), Some(4));
                assert_eq!(v["accents"].as_array().map(Vec::len), Some(7));
            }
            if entry.dst.ends_with(".toml") {
                toml::from_str::<toml::Value>(&out).unwrap_or_else(|e| {
                    panic!("{} ({mode}, {accent}, {level}) não é TOML válido: {e}\n{out}", entry.src)
                });
            }
        }
    }
}

#[test]
fn every_template_in_the_directory_is_in_the_manifest() {
    let paths = paths();
    let manifest = Manifest::load(&paths).unwrap();
    let listed: std::collections::HashSet<_> = manifest.template.iter().map(|t| t.src.clone()).collect();

    let mut found = Vec::new();
    fn walk(dir: &Path, base: &Path, out: &mut Vec<String>) {
        for e in std::fs::read_dir(dir).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                walk(&p, base, out);
            } else if p.extension().is_some_and(|x| x == "j2") {
                out.push(p.strip_prefix(base).unwrap().to_string_lossy().into_owned());
            }
        }
    }
    walk(&paths.templates_dir(), &paths.templates_dir(), &mut found);
    for f in found {
        assert!(listed.contains(&f), "{f} existe mas não está no manifest.toml (ninguém vai renderizar)");
    }
}

#[test]
fn light_and_dark_outputs_actually_differ() {
    let paths = paths();
    let tokens = Tokens::builtin();
    let src = std::fs::read_to_string(paths.templates_dir().join("foot/colors.ini.j2")).unwrap();
    let d = Theme::resolve(&tokens, Mode::Dark, "ember", MotionLevel::Full).unwrap();
    let l = Theme::resolve(&tokens, Mode::Light, "ember", MotionLevel::Full).unwrap();
    let (d, l) = (template::render("foot", &src, &d).unwrap(), template::render("foot", &src, &l).unwrap());
    assert!(d.contains("[colors-dark]") && d.contains("initial-color-theme=dark"));
    assert!(l.contains("[colors-light]") && l.contains("initial-color-theme=light"));
    assert!(d.contains("background=000000") && l.contains("background=FFFFFF"));
}
