//! Leitura mínima de entradas `.desktop` (freedesktop): só o que o hub precisa.

use std::collections::HashSet;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    /// `org.mozilla.firefox`, sem `.desktop`.
    pub id: String,
    pub name: String,
    pub exec: String,
    pub terminal: bool,
    pub keywords: String,
}

/// `lang` em ordem de preferência: `["pt_BR", "pt"]`.
pub fn parse(id: &str, text: &str, lang: &[String]) -> Option<Entry> {
    let mut in_group = false;
    let mut name = None;
    let mut localized: Vec<(usize, String)> = Vec::new();
    let (mut exec, mut keywords, mut kind) = (None, String::new(), None);
    let (mut terminal, mut hidden) = (false, false);

    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            // Só o grupo principal; ações extras ficam de fora.
            if in_group {
                break;
            }
            in_group = line == "[Desktop Entry]";
            continue;
        }
        if !in_group || line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else { continue };
        let (key, value) = (key.trim(), value.trim());
        match key {
            "Name" => name = Some(value.to_string()),
            "Exec" => exec = Some(value.to_string()),
            "Type" => kind = Some(value.to_string()),
            "Terminal" => terminal = value == "true",
            "NoDisplay" | "Hidden" => hidden |= value == "true",
            "Keywords" => keywords.push_str(&value.replace(';', " ")),
            _ => {
                if let Some(l) = key.strip_prefix("Name[").and_then(|k| k.strip_suffix(']')) {
                    if let Some(rank) = lang.iter().position(|x| x == l) {
                        localized.push((rank, value.to_string()));
                    }
                } else if let Some(l) = key.strip_prefix("Keywords[").and_then(|k| k.strip_suffix(']')) {
                    if lang.iter().any(|x| x == l) {
                        keywords.push(' ');
                        keywords.push_str(&value.replace(';', " "));
                    }
                }
            }
        }
    }

    if hidden || kind.as_deref() != Some("Application") {
        return None;
    }
    localized.sort_by_key(|(rank, _)| *rank);
    let name = localized.into_iter().next().map(|(_, n)| n).or(name)?;
    Some(Entry { id: id.to_string(), name, exec: exec?, terminal, keywords: keywords.trim().to_string() })
}

/// Remove os códigos de campo (`%f`, `%U`, ...) que só fazem sentido para um gerenciador de arquivos.
pub fn strip_field_codes(exec: &str) -> String {
    let mut out = String::with_capacity(exec.len());
    let mut chars = exec.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '%' {
            out.push(c);
            continue;
        }
        // `%%` é um `%` literal; qualquer outro código (%f, %U, ...) some.
        if chars.next() == Some('%') {
            out.push('%');
        }
    }
    // Remover um código pode deixar espaços duplos ou sobrando.
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

pub fn languages() -> Vec<String> {
    let raw = env::var("LC_MESSAGES").or_else(|_| env::var("LANG")).unwrap_or_default();
    languages_from(&raw)
}

/// `pt_BR.UTF-8` vira `["pt_BR", "pt"]`. `C` e `POSIX` não têm tradução.
pub fn languages_from(raw: &str) -> Vec<String> {
    let base = raw.split('.').next().unwrap_or("").split('@').next().unwrap_or("");
    let mut out = Vec::new();
    if !base.is_empty() && base != "C" && base != "POSIX" {
        out.push(base.to_string());
        if let Some((short, _)) = base.split_once('_') {
            out.push(short.to_string());
        }
    }
    out
}

fn app_dirs() -> Vec<PathBuf> {
    let home = env::var_os("HOME").map(PathBuf::from);
    let data_home = env::var_os("XDG_DATA_HOME").map(PathBuf::from).or_else(|| home.map(|h| h.join(".local/share")));
    let data_dirs = env::var("XDG_DATA_DIRS").unwrap_or_else(|_| "/usr/local/share:/usr/share".into());

    let mut dirs: Vec<PathBuf> = data_home.into_iter().collect();
    dirs.extend(data_dirs.split(':').filter(|s| !s.is_empty()).map(PathBuf::from));
    dirs.into_iter().map(|d| d.join("applications")).collect()
}

/// Todas as entradas visíveis. A primeira ocorrência de cada id vence (o diretório do usuário ganha).
pub fn load_all() -> Vec<Entry> {
    let lang = languages();
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for dir in app_dirs() {
        collect(&dir, &dir, &lang, &mut seen, &mut out);
    }
    out.sort_by_key(|e| e.name.to_lowercase());
    out
}

fn collect(base: &Path, dir: &Path, lang: &[String], seen: &mut HashSet<String>, out: &mut Vec<Entry>) {
    let Ok(read) = fs::read_dir(dir) else { return };
    for e in read.flatten() {
        let path = e.path();
        if path.is_dir() {
            collect(base, &path, lang, seen, out);
            continue;
        }
        if path.extension().is_none_or(|x| x != "desktop") {
            continue;
        }
        let rel = path.strip_prefix(base).unwrap_or(&path).with_extension("");
        let id = rel.to_string_lossy().replace('/', "-");
        if !seen.insert(id.clone()) {
            continue;
        }
        if let Some(entry) = fs::read_to_string(&path).ok().and_then(|t| parse(&id, &t, lang)) {
            out.push(entry);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIREFOX: &str = "\
[Desktop Entry]
Name=Firefox
Name[pt_BR]=Firefox (navegador)
Exec=firefox %u
Terminal=false
Type=Application
Keywords=web;browser;internet;
Keywords[pt_BR]=navegador;

[Desktop Action new-window]
Name=Nova janela
Exec=firefox --new-window
";

    fn pt() -> Vec<String> {
        vec!["pt_BR".into(), "pt".into()]
    }

    #[test]
    fn parses_name_exec_and_keywords() {
        let e = parse("firefox", FIREFOX, &[]).unwrap();
        assert_eq!(e.name, "Firefox");
        assert_eq!(e.exec, "firefox %u");
        assert!(!e.terminal);
        assert_eq!(e.keywords, "web browser internet");
    }

    #[test]
    fn prefers_localized_name_and_keywords() {
        let e = parse("firefox", FIREFOX, &pt()).unwrap();
        assert_eq!(e.name, "Firefox (navegador)");
        assert!(e.keywords.contains("navegador"));
    }

    #[test]
    fn ignores_action_groups() {
        // O Exec do grupo [Desktop Action ...] não pode sobrescrever o principal.
        assert_eq!(parse("f", FIREFOX, &[]).unwrap().exec, "firefox %u");
    }

    #[test]
    fn hidden_and_non_apps_are_skipped() {
        assert!(parse("x", "[Desktop Entry]\nName=X\nExec=x\nType=Application\nNoDisplay=true\n", &[]).is_none());
        assert!(parse("x", "[Desktop Entry]\nName=X\nExec=x\nType=Link\n", &[]).is_none());
        assert!(parse("x", "[Desktop Entry]\nName=X\nType=Application\n", &[]).is_none(), "sem Exec");
    }

    #[test]
    fn terminal_flag() {
        let e = parse("v", "[Desktop Entry]\nName=Vim\nExec=vim %F\nTerminal=true\nType=Application\n", &[]).unwrap();
        assert!(e.terminal);
    }

    #[test]
    fn strips_field_codes() {
        assert_eq!(strip_field_codes("firefox %u"), "firefox");
        assert_eq!(
            strip_field_codes("mpv --player-operation-mode=pseudo-gui -- %U"),
            "mpv --player-operation-mode=pseudo-gui --"
        );
        assert_eq!(strip_field_codes("sh -c 'echo 100%%'"), "sh -c 'echo 100%'");
        assert_eq!(strip_field_codes("app %i %c %k"), "app");
    }

    #[test]
    fn language_list_from_locale() {
        assert_eq!(languages_from("pt_BR.UTF-8"), vec!["pt_BR", "pt"]);
        assert_eq!(languages_from("de_DE@euro"), vec!["de_DE", "de"]);
        assert_eq!(languages_from("en"), vec!["en"]);
        assert!(languages_from("C").is_empty());
        assert!(languages_from("POSIX").is_empty());
        assert!(languages_from("").is_empty());
    }
}
