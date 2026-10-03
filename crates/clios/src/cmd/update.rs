//! `clios update`: lê as notícias do Arch antes de atualizar, atualiza, e avisa dos `.pacnew`.
//!
//! O Arch às vezes publica uma notícia pedindo uma intervenção manual antes de uma atualização.
//! Quem só roda `pacman -Syu` descobre depois que o sistema não sobe. Aqui a notícia vem primeiro.

use std::fs;
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};

use crate::ctx::Ctx;
use crate::sys::is_installed;
use crate::ui;

const FEED: &str = "https://archlinux.org/feeds/news/";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct News {
    pub title: String,
    pub link: String,
    /// Segundos desde 1970 (UTC).
    pub date: i64,
    pub summary: String,
}

impl News {
    /// A notícia pede algo do usuário antes da atualização?
    pub fn needs_manual_intervention(&self) -> bool {
        let t = format!("{} {}", self.title, self.summary).to_lowercase();
        ["manual intervention", "requires manual", "require manual", "intervenção manual"].iter().any(|k| t.contains(k))
    }
}

// ── parser do RSS (sem biblioteca de XML: o feed é pequeno e regular) ───────

fn unescape(s: &str) -> String {
    let s = s.trim();
    let s = s.strip_prefix("<![CDATA[").and_then(|r| r.strip_suffix("]]>")).unwrap_or(s);
    s.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&apos;", "'")
        .replace("&amp;", "&")
}

fn tag<'a>(item: &'a str, name: &str) -> Option<&'a str> {
    let open = format!("<{name}>");
    let close = format!("</{name}>");
    let start = item.find(&open)? + open.len();
    let end = item[start..].find(&close)? + start;
    Some(&item[start..end])
}

/// Tira as tags de um trecho de HTML, desfaz as entidades e junta os espaços.
pub fn strip_html(html: &str) -> String {
    let mut out = String::new();
    let mut in_tag = false;
    for ch in html.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => {
                in_tag = false;
                out.push(' ');
            }
            c if !in_tag => out.push(c),
            _ => {}
        }
    }
    unescape(&out).split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Dias desde 1970-01-01 para uma data do calendário gregoriano (algoritmo de Howard Hinnant).
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// `Tue, 14 Oct 2025 08:00:00 +0000` (RFC 2822) em segundos Unix.
pub fn parse_rfc2822(s: &str) -> Option<i64> {
    let s = s.trim();
    let s = s.split_once(',').map_or(s, |(_, r)| r).trim();
    let mut it = s.split_whitespace();
    let day: i64 = it.next()?.parse().ok()?;
    let name = it.next()?;
    let month = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"]
        .iter()
        .position(|m| m.eq_ignore_ascii_case(name))? as i64
        + 1;
    let year: i64 = it.next()?.parse().ok()?;
    let mut hms = it.next()?.split(':');
    let (h, mi): (i64, i64) = (hms.next()?.parse().ok()?, hms.next()?.parse().ok()?);
    let sec: i64 = hms.next().and_then(|s| s.parse().ok()).unwrap_or(0);
    let zone = it.next().unwrap_or("+0000");
    let offset = match zone {
        "GMT" | "UT" | "UTC" | "Z" => 0,
        z if z.len() == 5 && (z.starts_with('+') || z.starts_with('-')) => {
            let v: i64 = z[1..3].parse::<i64>().ok()? * 3600 + z[3..5].parse::<i64>().ok()? * 60;
            if z.starts_with('-') { -v } else { v }
        }
        _ => 0,
    };
    Some(days_from_civil(year, month, day) * 86_400 + h * 3600 + mi * 60 + sec - offset)
}

pub fn parse_feed(xml: &str) -> Vec<News> {
    let mut out = Vec::new();
    let mut rest = xml;
    while let Some(start) = rest.find("<item>") {
        let Some(len) = rest[start..].find("</item>") else { break };
        let item = &rest[start..start + len];
        rest = &rest[start + len + 7..];
        let (Some(title), Some(date)) = (tag(item, "title"), tag(item, "pubDate").and_then(parse_rfc2822)) else {
            continue;
        };
        out.push(News {
            title: unescape(title).split_whitespace().collect::<Vec<_>>().join(" "),
            link: tag(item, "link").map(unescape).unwrap_or_default(),
            date,
            // a descrição vem como HTML escapado: desfaz o escape, tira as tags, desfaz as entidades de dentro
            summary: strip_html(&unescape(tag(item, "description").unwrap_or(""))),
        });
    }
    out
}

/// As notícias que o usuário ainda não viu: as publicadas depois da última atualização,
/// ou as 3 mais recentes se nunca atualizou por aqui. As mais novas primeiro.
pub fn unread(mut news: Vec<News>, last_update: Option<i64>) -> Vec<News> {
    news.sort_by_key(|n| std::cmp::Reverse(n.date));
    match last_update {
        Some(t) => news.into_iter().filter(|n| n.date > t).collect(),
        None => news.into_iter().take(3).collect(),
    }
}

// ── .pacnew ───────────────────────────────────────────────────────────────

/// Procura `*.pacnew` e `*.pacsave` sob `dir` (profundidade limitada: /etc é raso).
pub fn find_pacnew(dir: &Path, depth: u32) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let Ok(rd) = fs::read_dir(dir) else { return found };
    for e in rd.flatten() {
        let p = e.path();
        let name = e.file_name().to_string_lossy().into_owned();
        if p.is_dir() && !p.is_symlink() {
            if depth > 0 {
                found.extend(find_pacnew(&p, depth - 1));
            }
        } else if name.ends_with(".pacnew") || name.ends_with(".pacsave") {
            found.push(p);
        }
    }
    found.sort();
    found
}

// ── a atualização ─────────────────────────────────────────────────────────

fn stamp_file(ctx: &Ctx) -> PathBuf {
    ctx.paths.state.join("last-update")
}

fn now() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs() as i64)
}

fn last_update(ctx: &Ctx) -> Option<i64> {
    fs::read_to_string(stamp_file(ctx)).ok()?.trim().parse().ok()
}

fn fetch_feed() -> Result<String> {
    let out = Command::new("curl").args(["-fsSL", "--max-time", "10", FEED]).output().context("curl não encontrado")?;
    if !out.status.success() {
        anyhow::bail!("{}", String::from_utf8_lossy(&out.stderr).trim());
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// Pergunta sim ou não. `default` vale para o Enter vazio.
pub(crate) fn ask(question: &str, default: bool) -> bool {
    let hint = if default { "[S/n]" } else { "[s/N]" };
    print!("{question} {hint} ");
    let _ = std::io::stdout().flush();
    let mut line = String::new();
    if std::io::stdin().lock().read_line(&mut line).is_err() {
        return false;
    }
    match line.trim().to_lowercase().as_str() {
        "" => default,
        "s" | "sim" | "y" | "yes" => true,
        _ => false,
    }
}

fn print_news(n: &News) {
    let flag = if n.needs_manual_intervention() { "\x1b[31m!\x1b[0m " } else { "  " };
    println!("{flag}{}", ui::bold(&n.title));
    if !n.summary.is_empty() {
        let short: String = n.summary.chars().take(220).collect();
        println!("  {}", ui::dim(&format!("{short}{}", if n.summary.chars().count() > 220 { "…" } else { "" })));
    }
    println!("  {}\n", ui::dim(&n.link));
}

pub fn run(ctx: &Ctx, news_only: bool, yes: bool) -> Result<bool> {
    println!("{}", ui::bold("notícias do Arch"));
    let feed = fetch_feed().map(|x| parse_feed(&x));
    let mut blocked = false;
    match feed {
        Ok(items) => {
            let fresh = unread(items, last_update(ctx));
            if fresh.is_empty() {
                println!("{}\n", ui::dim("nada novo desde a última atualização"));
            }
            for n in &fresh {
                print_news(n);
            }
            blocked = fresh.iter().any(News::needs_manual_intervention);
        }
        Err(e) => {
            println!("{}\n", ui::dim(&format!("não consegui ler as notícias ({e:#}). Confira em archlinux.org/news")))
        }
    }
    if news_only {
        return Ok(true);
    }

    if blocked {
        println!("\x1b[31mhá notícia pedindo intervenção manual.\x1b[0m Leia antes de seguir.");
    }
    if !yes && !ask("atualizar agora?", !blocked) {
        println!("{}", ui::dim("nada foi alterado"));
        return Ok(true);
    }

    let snaps = super::snap::unavailable().is_none();
    if snaps {
        println!(
            "{}",
            ui::dim("o snap-pac fotografa o sistema antes e depois; `clios snap undo` desfaz se der errado")
        );
    }
    let ok = if is_installed("paru") {
        Command::new("paru").arg("-Syu").status()
    } else {
        Command::new("sudo").args(["pacman", "-Syu"]).status()
    }
    .context("rodando a atualização")?
    .success();

    if !ok {
        println!("\n\x1b[31ma atualização não terminou.\x1b[0m O carimbo da última atualização não foi alterado.");
        return Ok(false);
    }
    let _ = clios_core::fsutil::write_atomic(&stamp_file(ctx), format!("{}\n", now()).as_bytes());

    if super::snap::reboot_pending() {
        println!(
            "\n{}",
            ui::bold("o kernel foi atualizado: reinicie quando puder (até lá, módulos novos não carregam).")
        );
    }
    if snaps {
        println!("{}", ui::dim("`clios snap diff` mostra o que esta atualização mudou"));
    }
    let pacnew = find_pacnew(Path::new("/etc"), 3);
    if pacnew.is_empty() {
        println!("\n{}", ui::bold("pronto."));
    } else {
        println!("\n{}", ui::bold("pronto, mas há arquivos de configuração novos para conferir:"));
        for p in &pacnew {
            println!("  {}", p.display());
        }
        println!("{}", ui::dim("rode `sudo pacdiff` (pacote pacman-contrib) para comparar e juntar"));
    }
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    const FEED_XML: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<rss version="2.0"><channel><title>Arch Linux: Recent news updates</title>
<item>
<title>Plasma 6.4 will need manual intervention</title>
<link>https://archlinux.org/news/plasma-6-4/</link>
<description>&lt;p&gt;Before upgrading, run &lt;code&gt;pacman -S foo&lt;/code&gt; &amp;amp; reboot.&lt;/p&gt;</description>
<pubDate>Tue, 14 Oct 2025 08:00:00 +0000</pubDate>
</item>
<item>
<title><![CDATA[Nova imagem do instalador]]></title>
<link>https://archlinux.org/news/instalador/</link>
<description>&lt;p&gt;Já disponível.&lt;/p&gt;</description>
<pubDate>Wed, 01 Oct 2025 10:30:00 -0300</pubDate>
</item>
<item>
<title>Sem data</title>
</item>
</channel></rss>"#;

    #[test]
    fn rfc2822_dates_become_unix_time() {
        assert_eq!(parse_rfc2822("Thu, 01 Jan 1970 00:00:00 +0000"), Some(0));
        assert_eq!(parse_rfc2822("Tue, 14 Oct 2025 08:00:00 +0000"), Some(1_760_428_800));
        // -0300: três horas depois em UTC
        assert_eq!(parse_rfc2822("Wed, 01 Oct 2025 10:30:00 -0300"), Some(1_759_325_400));
        assert_eq!(parse_rfc2822("14 Oct 2025 08:00 GMT"), Some(1_760_428_800));
        assert_eq!(parse_rfc2822("ontem"), None);
        assert_eq!(parse_rfc2822("Tue, 14 Foo 2025 08:00:00 +0000"), None);
    }

    #[test]
    fn feed_parses_items_unescapes_html_and_skips_broken_ones() {
        let news = parse_feed(FEED_XML);
        assert_eq!(news.len(), 2, "o item sem data não vale");
        assert_eq!(news[0].title, "Plasma 6.4 will need manual intervention");
        assert_eq!(news[0].link, "https://archlinux.org/news/plasma-6-4/");
        assert_eq!(news[0].summary, "Before upgrading, run pacman -S foo & reboot.");
        assert_eq!(news[1].title, "Nova imagem do instalador", "CDATA");
        assert_eq!(news[1].summary, "Já disponível.");
    }

    #[test]
    fn manual_intervention_is_detected_in_title_or_body() {
        let news = parse_feed(FEED_XML);
        assert!(news[0].needs_manual_intervention());
        assert!(!news[1].needs_manual_intervention());
        let body = News {
            title: "x".into(),
            link: String::new(),
            date: 0,
            summary: "This update requires manual intervention.".into(),
        };
        assert!(body.needs_manual_intervention());
    }

    #[test]
    fn unread_news_follow_the_last_update() {
        let news = parse_feed(FEED_XML);
        let after_first = unread(news.clone(), Some(1_759_400_000));
        assert_eq!(after_first.len(), 1);
        assert!(after_first[0].title.starts_with("Plasma"));
        assert!(unread(news.clone(), Some(1_800_000_000)).is_empty());
        // nunca atualizou: mostra as mais recentes, a mais nova primeiro
        let first_time = unread(news, None);
        assert_eq!(first_time.len(), 2);
        assert!(first_time[0].date > first_time[1].date);
    }

    #[test]
    fn html_stripping_keeps_text_with_sane_spacing() {
        assert_eq!(strip_html("<p>um</p><p>dois   <b>três</b></p>"), "um dois três");
        assert_eq!(strip_html("a &lt;b&gt; c"), "a <b> c");
    }

    #[test]
    fn pacnew_search_is_shallow_and_sorted() {
        let d = std::env::temp_dir().join(format!("clios-pacnew-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(d.join("a/b/c/d")).unwrap();
        fs::write(d.join("pacman.conf.pacnew"), "").unwrap();
        fs::write(d.join("a/x.conf.pacsave"), "").unwrap();
        fs::write(d.join("a/b/y.pacnew"), "").unwrap();
        fs::write(d.join("a/b/c/d/fundo.pacnew"), "").unwrap();
        fs::write(d.join("normal.conf"), "").unwrap();
        let found = find_pacnew(&d, 2);
        let names: Vec<_> = found.iter().map(|p| p.file_name().unwrap().to_string_lossy().into_owned()).collect();
        // ordenado pelo caminho completo: a/b/y, a/x, pacman; e c/d (fundo demais) fica de fora
        assert_eq!(names, ["y.pacnew", "x.conf.pacsave", "pacman.conf.pacnew"], "{names:?}");
    }
}
