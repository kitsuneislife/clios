//! De onde vêm as linhas do hub: catálogo, apps, janelas, atalhos, clipboard.

use std::process::{Command, Stdio};

use clios_core::{Paths, Theme};
use serde::Deserialize;

use super::desktop;
use super::items::{Action, Item, Kind, Scope, builtin_actions};

const DEFAULT_CATALOG: &str = include_str!("../../../../config/clios/hub.toml");

#[derive(Debug, Default, Deserialize)]
pub struct Catalog {
    #[serde(default)]
    pub tui: Vec<TuiDef>,
    #[serde(default)]
    pub cmd: Vec<CmdDef>,
}

#[derive(Debug, Deserialize)]
pub struct TuiDef {
    pub id: String,
    pub name: String,
    pub cmd: Vec<String>,
    #[serde(default)]
    pub keywords: String,
    #[serde(default)]
    pub float: bool,
    #[serde(default)]
    pub hold: bool,
}

#[derive(Debug, Deserialize)]
pub struct CmdDef {
    pub id: String,
    pub name: String,
    pub shell: String,
    #[serde(default)]
    pub keywords: String,
}

/// O catálogo do usuário se existir e for válido; senão o embutido. O aviso diz por quê.
pub fn load_catalog(paths: &Paths) -> (Catalog, Option<String>) {
    let user = paths.config.join("clios/hub.toml");
    if let Ok(text) = std::fs::read_to_string(&user) {
        match toml::from_str::<Catalog>(&text) {
            Ok(c) => return (c, None),
            Err(e) => {
                let fallback = toml::from_str(DEFAULT_CATALOG).unwrap_or_default();
                let first = e.to_string().lines().next().unwrap_or_default().to_string();
                return (fallback, Some(format!("hub.toml inválido, usando o padrão: {first}")));
            }
        }
    }
    (toml::from_str(DEFAULT_CATALOG).expect("catálogo embutido precisa ser válido"), None)
}

/// Linhas que não mudam enquanto o hub está aberto.
pub fn static_items(catalog: &Catalog, theme: &Theme) -> Vec<Item> {
    let mut v = Vec::new();

    for t in &catalog.tui {
        v.push(
            Item::new(
                Kind::Tui,
                format!("tui:{}", t.id),
                t.name.clone(),
                Action::Tui { id: t.id.clone(), argv: t.cmd.clone(), float: t.float, hold: t.hold },
            )
            .keywords(t.keywords.clone()),
        );
    }
    for c in &catalog.cmd {
        // O `clip` do catálogo é só um atalho para o escopo de clipboard.
        let action = if c.id == "clip" { Action::Scope(Scope::Clipboard) } else { Action::Shell(c.shell.clone()) };
        v.push(Item::new(Kind::Action, format!("cmd:{}", c.id), c.name.clone(), action).keywords(c.keywords.clone()));
    }

    for e in desktop::load_all() {
        let cmd = desktop::strip_field_codes(&e.exec);
        let action = if e.terminal {
            Action::Tui { id: e.id.clone(), argv: vec!["sh".into(), "-c".into(), cmd], float: false, hold: false }
        } else {
            Action::Gui(cmd)
        };
        let kind = if e.terminal { Kind::Tui } else { Kind::App };
        v.push(Item::new(kind, format!("app:{}", e.id), e.name, action).keywords(e.keywords));
    }

    v.extend(builtin_actions(theme));
    v
}

fn run_capture(prog: &str, args: &[&str]) -> Option<String> {
    let out = Command::new(prog).args(args).stderr(Stdio::null()).output().ok()?;
    out.status.success().then(|| String::from_utf8_lossy(&out.stdout).into_owned())
}

pub fn windows() -> Vec<Item> {
    run_capture("hyprctl", &["clients", "-j"]).map(|s| windows_from_json(&s)).unwrap_or_default()
}

pub fn keys() -> Vec<Item> {
    run_capture("hyprctl", &["binds", "-j"]).map(|s| keys_from_json(&s)).unwrap_or_default()
}

pub fn clipboard() -> Vec<Item> {
    run_capture("cliphist", &["list"]).map(|s| clips_from_list(&s)).unwrap_or_default()
}

pub fn windows_from_json(json: &str) -> Vec<Item> {
    let Ok(serde_json::Value::Array(list)) = serde_json::from_str::<serde_json::Value>(json) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for w in list {
        let addr = w["address"].as_str().unwrap_or_default();
        let class = w["class"].as_str().unwrap_or_default();
        let mapped = w["mapped"].as_bool().unwrap_or(true);
        let hidden = w["hidden"].as_bool().unwrap_or(false);
        if addr.is_empty() || !mapped || hidden || class == "clios.hub" {
            continue;
        }
        let title = w["title"].as_str().filter(|t| !t.is_empty()).unwrap_or(class);
        let ws = w["workspace"]["name"].as_str().unwrap_or("?");
        let hint = format!("ws {ws}");
        out.push(
            Item::new(
                Kind::Window,
                format!("win:{addr}"),
                title,
                Action::Hypr(format!("hl.dsp.focus({{ window = \"address:{addr}\" }})")),
            )
            .hint(hint)
            .keywords(class),
        );
    }
    out
}

/// `SHIFT=1, CTRL=4, ALT=8, SUPER=64` (máscara do xkb que o Hyprland repassa).
pub fn format_combo(modmask: u64, key: &str) -> String {
    let mut parts: Vec<String> = Vec::new();
    if modmask & 64 != 0 {
        parts.push("super".into());
    }
    if modmask & 4 != 0 {
        parts.push("ctrl".into());
    }
    if modmask & 8 != 0 {
        parts.push("alt".into());
    }
    if modmask & 1 != 0 {
        parts.push("shift".into());
    }
    parts.push(match key {
        "mouse:272" => "botão esquerdo".to_string(),
        "mouse:273" => "botão direito".to_string(),
        "mouse_down" => "scroll ↓".to_string(),
        "mouse_up" => "scroll ↑".to_string(),
        k => k.to_lowercase(),
    });
    parts.join(" + ")
}

pub fn keys_from_json(json: &str) -> Vec<Item> {
    let Ok(serde_json::Value::Array(list)) = serde_json::from_str::<serde_json::Value>(json) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for (i, b) in list.iter().enumerate() {
        // Só o que tem descrição: atalho sem descrição é implementação, não documentação.
        let desc = b["description"].as_str().unwrap_or_default();
        if desc.is_empty() {
            continue;
        }
        let combo = format_combo(b["modmask"].as_u64().unwrap_or(0), b["key"].as_str().unwrap_or_default());
        let submap = b["submap"].as_str().unwrap_or_default();
        let hint = if submap.is_empty() { combo } else { format!("[{submap}] {combo}") };
        out.push(Item::new(Kind::Key, format!("key:{i}"), desc, Action::None).hint(hint));
    }
    out
}

pub fn clips_from_list(list: &str) -> Vec<Item> {
    list.lines()
        .take(300)
        .filter_map(|line| {
            let (id, preview) = line.split_once('\t')?;
            let text: String = preview.split_whitespace().collect::<Vec<_>>().join(" ");
            if text.is_empty() {
                return None;
            }
            let title: String = text.chars().take(200).collect();
            Some(Item::new(Kind::Clip, format!("clip:{id}"), title, Action::Clip(id.to_string())))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_catalog_is_valid_and_complete() {
        let c: Catalog = toml::from_str(DEFAULT_CATALOG).unwrap();
        assert!(c.tui.len() >= 10);
        let mut ids: Vec<_> = c.tui.iter().map(|t| t.id.as_str()).chain(c.cmd.iter().map(|c| c.id.as_str())).collect();
        let n = ids.len();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), n, "ids duplicados no catálogo");
        for t in &c.tui {
            assert!(!t.cmd.is_empty(), "{} sem comando", t.id);
        }
    }

    #[test]
    fn windows_skip_hidden_unmapped_and_the_hub_itself() {
        let json = r#"[
          {"address":"0xaa","mapped":true,"hidden":false,"class":"firefox","title":"Docs","workspace":{"id":2,"name":"2"}},
          {"address":"0xbb","mapped":true,"hidden":false,"class":"clios.hub","title":"hub","workspace":{"id":2,"name":"2"}},
          {"address":"0xcc","mapped":false,"hidden":false,"class":"foot","title":"x","workspace":{"id":1,"name":"1"}},
          {"address":"0xdd","mapped":true,"hidden":true,"class":"foot","title":"y","workspace":{"id":1,"name":"1"}},
          {"address":"0xee","mapped":true,"hidden":false,"class":"foot","title":"","workspace":{"id":1,"name":"1"}}
        ]"#;
        let w = windows_from_json(json);
        assert_eq!(w.len(), 2);
        assert_eq!(w[0].title, "Docs");
        assert_eq!(w[0].hint, "ws 2");
        assert_eq!(w[1].title, "foot", "título vazio cai para a classe");
        assert_eq!(w[0].action, Action::Hypr("hl.dsp.focus({ window = \"address:0xaa\" })".into()));
    }

    #[test]
    fn garbage_json_is_an_empty_list_not_a_crash() {
        assert!(windows_from_json("não é json").is_empty());
        assert!(windows_from_json("{}").is_empty());
        assert!(keys_from_json("").is_empty());
    }

    #[test]
    fn combos_are_readable() {
        assert_eq!(format_combo(64, "Return"), "super + return");
        assert_eq!(format_combo(64 | 1, "Q"), "super + shift + q");
        assert_eq!(format_combo(64 | 4, "h"), "super + ctrl + h");
        assert_eq!(format_combo(64, "mouse:272"), "super + botão esquerdo");
        assert_eq!(format_combo(0, "Print"), "print");
    }

    #[test]
    fn keys_only_list_described_binds() {
        let json = r#"[
          {"modmask":64,"key":"Q","description":"fechar janela","submap":""},
          {"modmask":64,"key":"X","description":"","submap":""},
          {"modmask":0,"key":"h","description":"esquerda","submap":"resize"}
        ]"#;
        let k = keys_from_json(json);
        assert_eq!(k.len(), 2);
        assert_eq!(k[0].hint, "super + q");
        assert_eq!(k[1].hint, "[resize] h");
        assert!(k.iter().all(|i| i.action == Action::None));
    }

    #[test]
    fn clipboard_lines_become_items() {
        let list = "12\thello   world\n7\t\n3\t[[ binary data 1 KiB png 10x10 ]]\nsem-tab\n";
        let c = clips_from_list(list);
        assert_eq!(c.len(), 2);
        assert_eq!(c[0].title, "hello world");
        assert_eq!(c[0].action, Action::Clip("12".into()));
    }
}
