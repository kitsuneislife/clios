//! `clios snap`: fotografias do sistema (btrfs + snapper) e o caminho de volta.
//!
//! O bootstrap liga o snapper e o snap-pac quando a raiz é btrfs: cada transação do pacman ganha uma fotografia antes
//! e uma depois. Isto aqui é a parte que faltava na maioria das distros: ver o que cada atualização mudou e desfazer
//! uma delas com um comando, com o sistema rodando, sem pendrive.
//!
//! Desfazer usa o `snapper undochange`, que devolve os arquivos da raiz ao que eram. O que mora fora dela (o kernel no
//! /boot, que é FAT) não volta; por isso uma transação que trocou o kernel é recusada, com a explicação do caminho
//! certo (escolher a fotografia no menu de boot, ou só reiniciar).

use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result, bail};
use serde::Deserialize;

use crate::sys::is_installed;
use crate::ui;

const CONFIG: &str = "root";

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Snapshot {
    pub number: u32,
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(rename = "pre-number", default)]
    pub pre_number: Option<u32>,
    #[serde(default)]
    pub date: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub userdata: Option<std::collections::BTreeMap<String, String>>,
}

/// Uma linha da lista: uma fotografia avulsa, ou o par antes/depois de uma transação.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub number: u32,
    pub post: Option<u32>,
    pub date: String,
    pub description: String,
    pub important: bool,
}

/// `{"root":[...]}` do `snapper --jsonout list`.
pub fn parse_list(json: &str) -> Result<Vec<Snapshot>> {
    let v: serde_json::Value = serde_json::from_str(json).context("a resposta do snapper não é JSON")?;
    let arr = v.get(CONFIG).cloned().unwrap_or(serde_json::Value::Array(vec![]));
    serde_json::from_value(arr).context("lendo a lista do snapper")
}

/// Junta os pares pre/post e ignora a fotografia 0 (o sistema atual). A mais nova vem primeiro.
pub fn entries(snaps: &[Snapshot]) -> Vec<Entry> {
    let mut out: Vec<Entry> = Vec::new();
    for s in snaps.iter().filter(|s| s.number != 0) {
        let important = s.userdata.as_ref().is_some_and(|u| u.get("important").is_some_and(|v| v == "yes"));
        match s.kind.as_str() {
            "post" => {
                if let Some(e) = s.pre_number.and_then(|p| out.iter_mut().find(|e| e.number == p)) {
                    e.post = Some(s.number);
                    e.important |= important;
                    continue;
                }
                out.push(Entry {
                    number: s.number,
                    post: None,
                    date: s.date.clone(),
                    description: s.description.clone(),
                    important,
                })
            }
            _ => out.push(Entry {
                number: s.number,
                post: None,
                date: s.date.clone(),
                description: s.description.clone(),
                important,
            }),
        }
    }
    out.reverse();
    out
}

/// O intervalo a comparar ou desfazer: o par da transação, ou da fotografia até agora (0).
pub fn range(entries: &[Entry], n: u32) -> Option<(u32, u32)> {
    if let Some(e) = entries.iter().find(|e| e.number == n || e.post == Some(n)) {
        return Some((e.number, e.post.unwrap_or(0)));
    }
    None
}

/// Uma linha do `snapper status`: `c..... /usr/bin/x`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Change {
    /// `+` criado, `-` apagado, `c` conteúdo, `t` tipo, ou `.` para mudança só de metadados.
    pub what: char,
    pub path: String,
}

pub fn parse_status(out: &str) -> Vec<Change> {
    out.lines()
        .filter_map(|l| {
            let (flags, path) = l.split_once(' ')?;
            let what = flags.chars().next()?;
            path.starts_with('/').then(|| Change { what, path: path.to_string() })
        })
        .collect()
}

/// A transação mexeu no kernel? Os módulos ficam na raiz, a imagem fica no /boot: desfazer deixaria os dois
/// desencontrados e o próximo boot falharia.
pub fn touches_kernel(changes: &[Change]) -> bool {
    changes.iter().any(|c| c.path.starts_with("/usr/lib/modules/"))
}

/// Pacotes que entraram, saíram ou mudaram, pelo banco do pacman (`/var/lib/pacman/local/<nome>-<versão>/`).
pub fn packages(changes: &[Change]) -> (Vec<String>, Vec<String>) {
    let mut added = Vec::new();
    let mut removed = Vec::new();
    for c in changes {
        let Some(rest) = c.path.strip_prefix("/var/lib/pacman/local/") else { continue };
        if rest.contains('/') || rest.is_empty() {
            continue;
        }
        match c.what {
            '+' => added.push(rest.to_string()),
            '-' => removed.push(rest.to_string()),
            _ => {}
        }
    }
    (added, removed)
}

/// `nome-1.2-1` → `nome`; a versão do pacman é sempre os dois últimos campos.
pub fn pkg_name(dir: &str) -> &str {
    let mut it = dir.rmatch_indices('-');
    match (it.next(), it.next()) {
        (Some(_), Some((i, _))) => &dir[..i],
        _ => dir,
    }
}

/// `nome-1.0-1` e `nome-1.1-1` viram `nome 1.0-1 → 1.1-1`.
pub fn upgrades(added: &[String], removed: &[String]) -> Vec<String> {
    let mut out = Vec::new();
    for a in added {
        let name = pkg_name(a);
        match removed.iter().find(|r| pkg_name(r) == name) {
            Some(r) => out.push(format!("{name} {} → {}", &r[name.len() + 1..], &a[name.len() + 1..])),
            None => out.push(format!("{name} {} (novo)", &a[name.len() + 1..])),
        }
    }
    for r in removed {
        let name = pkg_name(r);
        if !added.iter().any(|a| pkg_name(a) == name) {
            out.push(format!("{name} (removido)"));
        }
    }
    out.sort();
    out
}

// ── o sistema ─────────────────────────────────────────────────────────────

/// Por que não dá para fotografar aqui, em uma frase; `None` quando dá.
pub fn unavailable() -> Option<String> {
    let fs = Command::new("findmnt")
        .args(["-no", "FSTYPE", "/"])
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default();
    if !fs.is_empty() && fs != "btrfs" {
        return Some(format!(
            "a raiz é {fs}; as fotografias precisam de btrfs (no archinstall, escolha btrfs com os subvolumes padrão)"
        ));
    }
    if !is_installed("snapper") {
        return Some(
            "o snapper não está instalado: rode o bootstrap de novo (ele liga tudo quando a raiz é btrfs)".into(),
        );
    }
    if !Path::new("/etc/snapper/configs").join(CONFIG).exists() {
        return Some("o snapper não tem a configuração `root`: rode o bootstrap de novo".into());
    }
    None
}

/// Roda o snapper como você (o bootstrap libera o seu usuário em ALLOW_USERS) e, se for recusado, com sudo.
fn snapper(args: &[&str], sudo: bool) -> Result<std::process::Output> {
    let mut base = vec!["--iso", "-c", CONFIG];
    base.extend(args);
    let out = Command::new("snapper").args(&base).output().context("rodando o snapper")?;
    if out.status.success() || !sudo {
        return Ok(out);
    }
    Command::new("sudo").arg("snapper").args(&base).output().context("rodando o snapper com sudo")
}

fn list_entries() -> Result<Vec<Entry>> {
    let out = snapper(&["--jsonout", "list", "--disable-used-space"], true)?;
    if !out.status.success() {
        bail!("o snapper recusou: {}", String::from_utf8_lossy(&out.stderr).trim());
    }
    Ok(entries(&parse_list(&String::from_utf8_lossy(&out.stdout))?))
}

fn ensure() -> Result<()> {
    if let Some(why) = unavailable() {
        bail!("{why}");
    }
    Ok(())
}

pub fn list(count: usize) -> Result<()> {
    ensure()?;
    let es = list_entries()?;
    if es.is_empty() {
        println!("nenhuma fotografia ainda. A próxima atualização cria um par, ou `clios snap new`.");
        return Ok(());
    }
    for e in es.iter().take(count) {
        let id = match e.post {
            Some(p) => format!("{:>4}→{:<4}", e.number, p),
            None => format!("{:>4}     ", e.number),
        };
        let star = if e.important { " *" } else { "" };
        println!("{}  {}  {}{star}", ui::bold(&id), ui::dim(&e.date), e.description);
    }
    if es.len() > count {
        println!("{}", ui::dim(&format!("… e mais {}", es.len() - count)));
    }
    println!(
        "\n{}",
        ui::dim("`clios snap diff N` mostra o que mudou · `clios snap undo N` desfaz · * = mexeu no kernel ou no boot")
    );
    Ok(())
}

pub fn new(description: &str) -> Result<()> {
    ensure()?;
    let desc = if description.trim().is_empty() { "manual" } else { description.trim() };
    let out = snapper(&["create", "-p", "-c", "number", "-d", desc], true)?;
    if !out.status.success() {
        bail!("o snapper recusou: {}", String::from_utf8_lossy(&out.stderr).trim());
    }
    println!("fotografia {} criada", String::from_utf8_lossy(&out.stdout).trim());
    Ok(())
}

fn changes(a: u32, b: u32) -> Result<Vec<Change>> {
    let out = snapper(&["status", &format!("{a}..{b}")], true)?;
    if !out.status.success() {
        bail!("o snapper recusou: {}", String::from_utf8_lossy(&out.stderr).trim());
    }
    Ok(parse_status(&String::from_utf8_lossy(&out.stdout)))
}

fn label(a: u32, b: u32) -> String {
    if b == 0 { format!("da {a} até agora") } else { format!("{a}→{b}") }
}

pub fn diff(n: u32, all: bool) -> Result<()> {
    ensure()?;
    let es = list_entries()?;
    let Some((a, b)) = range(&es, n) else { bail!("não há fotografia {n} (veja `clios snap`)") };
    let ch = changes(a, b)?;
    let (added, removed) = packages(&ch);
    let pk = upgrades(&added, &removed);
    println!("{} {}", ui::bold(&label(a, b)), ui::dim(&format!("{} arquivos", ch.len())));
    if !pk.is_empty() {
        println!();
        for p in &pk {
            println!("  {p}");
        }
    }
    if all || pk.is_empty() {
        println!();
        for c in ch.iter().take(if all { usize::MAX } else { 40 }) {
            println!("  {} {}", c.what, c.path);
        }
        if !all && ch.len() > 40 {
            println!("{}", ui::dim(&format!("  … e mais {} (`--all` mostra tudo)", ch.len() - 40)));
        }
    }
    if touches_kernel(&ch) {
        println!("\n{}", ui::dim("esta mudança inclui o kernel: para voltar, escolha a fotografia no menu de boot"));
    }
    Ok(())
}

pub fn undo(n: u32, yes: bool) -> Result<()> {
    ensure()?;
    let es = list_entries()?;
    let Some((a, b)) = range(&es, n) else { bail!("não há fotografia {n} (veja `clios snap`)") };
    let ch = changes(a, b)?;
    if ch.is_empty() {
        println!("nada mudou {}: não há o que desfazer", label(a, b));
        return Ok(());
    }
    if touches_kernel(&ch) {
        bail!(
            "{} trocou o kernel, e o kernel do /boot não volta com a raiz: desfazer deixaria o sistema sem boot.\n\
             Para voltar, reinicie e escolha a fotografia {a} no menu de boot (com o limine-snapper-sync), ou\n\
             reinstale a versão anterior do pacote pelo cache: `sudo pacman -U /var/cache/pacman/pkg/linux-…`",
            label(a, b)
        );
    }
    let (added, removed) = packages(&ch);
    println!("{} devolve {} arquivos ao que eram.", ui::bold(&format!("desfazer {}", label(a, b))), ch.len());
    for p in upgrades(&added, &removed).iter().take(15) {
        println!("  {p}");
    }
    println!("{}", ui::dim("a sua pasta pessoal não é tocada. Antes, uma fotografia nova guarda o estado de agora."));
    if !yes && !super::update::ask("desfazer?", false) {
        println!("{}", ui::dim("nada foi alterado"));
        return Ok(());
    }
    let safety = snapper(&["create", "-p", "-c", "number", "-d", &format!("antes de desfazer {}", label(a, b))], true)?;
    if !safety.status.success() {
        bail!("não consegui criar a fotografia de segurança; nada foi alterado");
    }
    let st = Command::new("sudo")
        .args(["snapper", "-c", CONFIG, "undochange", &format!("{a}..{b}")])
        .status()
        .context("rodando o snapper undochange")?;
    if !st.success() {
        bail!("o snapper não terminou de desfazer. A fotografia {} guarda o estado de antes", stdout_num(&safety));
    }
    println!(
        "\n{} reinicie para os serviços pegarem os arquivos antigos. Se precisar voltar: `clios snap undo {}`.",
        ui::bold("pronto."),
        stdout_num(&safety)
    );
    Ok(())
}

fn stdout_num(o: &std::process::Output) -> String {
    String::from_utf8_lossy(&o.stdout).trim().to_string()
}

// ── reinício pendente ─────────────────────────────────────────────────────

/// O kernel em uso não tem mais os módulos no disco: uma atualização trocou o kernel e só um reinício o põe em uso
/// (até lá, um pendrive ou um módulo novo não carrega).
pub fn reboot_pending_in(modules: &Path, release: &str) -> bool {
    !release.is_empty() && modules.is_dir() && !modules.join(release).exists()
}

pub fn reboot_pending() -> bool {
    let release = std::fs::read_to_string("/proc/sys/kernel/osrelease").unwrap_or_default();
    reboot_pending_in(Path::new("/usr/lib/modules"), release.trim())
}

#[cfg(test)]
mod tests {
    use super::*;

    // Formato do `snapper --jsonout list` (client/snapper/cmd-list.cc): um array por configuração, nulos explícitos.
    const LIST: &str = r#"{"root":[
      {"number":0,"type":"single","pre-number":null,"date":"","user":"root","cleanup":"","description":"current","userdata":null},
      {"number":1,"type":"single","pre-number":null,"date":"2026-09-30 10:00:00","user":"root","cleanup":"","description":"bootstrap","userdata":null},
      {"number":2,"type":"pre","pre-number":null,"date":"2026-10-01 09:12:03","user":"root","cleanup":"number","description":"pacman -Syu","userdata":null},
      {"number":3,"type":"post","pre-number":2,"date":"2026-10-01 09:13:40","user":"root","cleanup":"number","description":"htop helix","userdata":null},
      {"number":4,"type":"pre","pre-number":null,"date":"2026-10-02 20:00:00","user":"root","cleanup":"number","description":"pacman -Syu","userdata":{"important":"yes"}},
      {"number":5,"type":"post","pre-number":4,"date":"2026-10-02 20:02:00","user":"root","cleanup":"number","description":"linux","userdata":{"important":"yes"}}
    ]}"#;

    #[test]
    fn pairs_are_joined_newest_first_and_current_is_hidden() {
        let es = entries(&parse_list(LIST).unwrap());
        assert_eq!(es.len(), 3);
        assert_eq!((es[0].number, es[0].post, es[0].important), (4, Some(5), true));
        assert_eq!((es[1].number, es[1].post, es[1].important), (2, Some(3), false));
        assert_eq!((es[2].number, es[2].post), (1, None));
        assert_eq!(es[1].description, "pacman -Syu");
    }

    #[test]
    fn a_range_is_the_pair_or_up_to_now() {
        let es = entries(&parse_list(LIST).unwrap());
        assert_eq!(range(&es, 2), Some((2, 3)));
        assert_eq!(range(&es, 3), Some((2, 3)));
        assert_eq!(range(&es, 1), Some((1, 0)));
        assert_eq!(range(&es, 99), None);
    }

    #[test]
    fn empty_or_other_configs_are_not_an_error() {
        assert!(parse_list(r#"{"home":[]}"#).unwrap().is_empty());
        assert!(parse_list("lixo").is_err());
    }

    const STATUS: &str = "c..... /usr/bin/hx\n\
+..... /var/lib/pacman/local/helix-25.07-1\n\
+..... /var/lib/pacman/local/helix-25.07-1/desc\n\
-..... /var/lib/pacman/local/helix-25.01-2\n\
+..... /var/lib/pacman/local/htop-3.4.1-1\n\
-..... /var/lib/pacman/local/nano-8.0-1\n\
....x. /etc/foo\n\
lixo sem caminho\n";

    #[test]
    fn status_lines_become_changes() {
        let ch = parse_status(STATUS);
        assert_eq!(ch.len(), 7);
        assert_eq!(ch[0], Change { what: 'c', path: "/usr/bin/hx".into() });
        assert_eq!(ch[6].what, '.');
        assert!(!touches_kernel(&ch));
    }

    #[test]
    fn packages_read_like_an_upgrade_list() {
        let (a, r) = packages(&parse_status(STATUS));
        assert_eq!(a, ["helix-25.07-1", "htop-3.4.1-1"]);
        assert_eq!(r, ["helix-25.01-2", "nano-8.0-1"]);
        assert_eq!(upgrades(&a, &r), ["helix 25.01-2 → 25.07-1", "htop 3.4.1-1 (novo)", "nano (removido)"]);
    }

    #[test]
    fn package_names_keep_their_own_hyphens() {
        assert_eq!(pkg_name("xdg-desktop-portal-hyprland-1.3.9-2"), "xdg-desktop-portal-hyprland");
        assert_eq!(pkg_name("linux-6.16.1.arch1-1"), "linux");
        assert_eq!(pkg_name("semversao"), "semversao");
    }

    #[test]
    fn a_kernel_change_is_detected_by_its_modules() {
        let ch =
            parse_status("+..... /usr/lib/modules/6.17.1-arch1-1/vmlinuz\n-..... /usr/lib/modules/6.16.9-arch1-1\n");
        assert!(touches_kernel(&ch));
    }

    #[test]
    fn reboot_is_pending_when_the_running_kernel_lost_its_modules() {
        let d = std::env::temp_dir().join(format!("clios-mods-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(d.join("6.17.1-arch1-1")).unwrap();
        assert!(!reboot_pending_in(&d, "6.17.1-arch1-1"));
        assert!(reboot_pending_in(&d, "6.16.9-arch1-1"));
        assert!(!reboot_pending_in(&d, ""));
        assert!(!reboot_pending_in(&d.join("nao-existe"), "x"));
    }
}
