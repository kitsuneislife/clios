//! `clios notifs`: o histórico de notificações.
//!
//! A shell (Quickshell) mostra o cartão e some com ele; o que passou durante o "não perturbe" nem aparece.
//! Cada notificação que chega é registrada aqui (`clios notifs add`, chamado pela shell), e este comando as lista.
//! O arquivo é `~/.local/state/clios/notifications.jsonl`, uma notificação por linha, com as últimas 200.

use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::ctx::Ctx;
use crate::ui;

/// Quantas ficam guardadas.
const KEEP: usize = 200;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entry {
    /// Segundos desde 1970.
    pub t: u64,
    pub app: String,
    pub summary: String,
    #[serde(default)]
    pub body: String,
    /// `low`, `normal` ou `critical`.
    #[serde(default)]
    pub urgency: String,
    /// Chegou durante o "não perturbe" e não foi mostrada.
    #[serde(default)]
    pub silenced: bool,
}

fn path(ctx: &Ctx) -> PathBuf {
    ctx.paths.state.join("notifications.jsonl")
}

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

/// `há 5 min`, `há 3 h`, `ontem`, `há 4 dias`.
pub fn ago(now: u64, t: u64) -> String {
    let d = now.saturating_sub(t);
    match d {
        0..=44 => "agora".into(),
        45..=3_599 => format!("há {} min", (d + 30) / 60),
        3_600..=86_399 => format!("há {} h", d / 3_600),
        86_400..=172_799 => "ontem".into(),
        _ => format!("há {} dias", d / 86_400),
    }
}

pub fn parse(text: &str) -> Vec<Entry> {
    text.lines().filter_map(|l| serde_json::from_str(l).ok()).collect()
}

/// Mantém só as últimas `KEEP`.
pub fn trim(mut v: Vec<Entry>) -> Vec<Entry> {
    if v.len() > KEEP {
        v.drain(..v.len() - KEEP);
    }
    v
}

pub fn add(ctx: &Ctx, e: Entry) -> Result<()> {
    let p = path(ctx);
    if let Some(dir) = p.parent() {
        fs::create_dir_all(dir)?;
    }
    let mut f = fs::OpenOptions::new().create(true).append(true).open(&p)?;
    writeln!(f, "{}", serde_json::to_string(&e)?)?;
    // Passou do dobro do limite: reescreve só com o que fica. Raro, e barato (400 linhas).
    if fs::metadata(&p).is_ok_and(|m| m.len() > 200_000) || parse(&fs::read_to_string(&p)?).len() > KEEP * 2 {
        let kept = trim(parse(&fs::read_to_string(&p)?));
        let body: String = kept.iter().filter_map(|e| serde_json::to_string(e).ok()).map(|l| l + "\n").collect();
        clios_core::fsutil::write_atomic(&p, body.as_bytes())?;
    }
    Ok(())
}

pub fn clear(ctx: &Ctx) -> Result<()> {
    let _ = fs::remove_file(path(ctx));
    println!("histórico limpo");
    Ok(())
}

/// Lista as últimas `n`, da mais nova para a mais velha.
pub fn list(ctx: &Ctx, n: usize) -> Result<()> {
    let all = fs::read_to_string(path(ctx)).map(|t| parse(&t)).unwrap_or_default();
    if all.is_empty() {
        println!("{}", ui::dim("nenhuma notificação por enquanto"));
        return Ok(());
    }
    let t = now();
    for e in all.iter().rev().take(n) {
        let when = ago(t, e.t);
        let mut head = format!("{:<10}  {:<14}  {}", ui::dim(&when), ui::dim(&e.app), ui::bold(&e.summary));
        if e.silenced {
            head.push_str(&ui::dim("  (silenciada)"));
        }
        if e.urgency == "critical" {
            head.push_str(&ui::dim("  (urgente)"));
        }
        println!("{head}");
        for line in e.body.lines().filter(|l| !l.trim().is_empty()).take(2) {
            println!("{:<10}  {:<14}  {}", "", "", ui::dim(line.trim()));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn e(t: u64, summary: &str) -> Entry {
        Entry {
            t,
            app: "app".into(),
            summary: summary.into(),
            body: String::new(),
            urgency: "normal".into(),
            silenced: false,
        }
    }

    #[test]
    fn times_read_like_a_person() {
        assert_eq!(ago(1_000, 990), "agora");
        assert_eq!(ago(1_000, 700), "há 5 min");
        assert_eq!(ago(100_000, 100_000 - 3 * 3_600 - 10), "há 3 h");
        assert_eq!(ago(200_000, 200_000 - 90_000), "ontem");
        assert_eq!(ago(1_000_000, 1_000_000 - 4 * 86_400), "há 4 dias");
        assert_eq!(ago(5, 100), "agora", "relógio andou para trás: não entra em pânico");
    }

    #[test]
    fn jsonl_roundtrips_and_skips_broken_lines() {
        let a = Entry { silenced: true, body: "linha 1\nlinha 2".into(), ..e(5, "oi \"mundo\"") };
        let text =
            format!("{}\nlixo\n{}\n", serde_json::to_string(&a).unwrap(), serde_json::to_string(&e(6, "b")).unwrap());
        let v = parse(&text);
        assert_eq!(v.len(), 2);
        assert_eq!(v[0], a);
    }

    #[test]
    fn old_entries_without_optional_fields_still_load() {
        let v = parse(r#"{"t":1,"app":"x","summary":"y"}"#);
        assert_eq!(v.len(), 1);
        assert!(!v[0].silenced && v[0].body.is_empty());
    }

    #[test]
    fn trim_keeps_the_newest() {
        let v: Vec<Entry> = (0..KEEP as u64 + 50).map(|i| e(i, "x")).collect();
        let t = trim(v);
        assert_eq!(t.len(), KEEP);
        assert_eq!(t[0].t, 50);
        assert_eq!(t.last().unwrap().t, KEEP as u64 + 49);
    }

    #[test]
    fn adding_appends_and_the_file_never_grows_without_bound() {
        let d = std::env::temp_dir().join(format!("clios-notifs-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let ctx = Ctx::load(Some(&root), Some(&d)).unwrap();
        for i in 0..(KEEP as u64 * 2 + 30) {
            add(&ctx, e(i, &format!("n{i}"))).unwrap();
        }
        let v = parse(&fs::read_to_string(path(&ctx)).unwrap());
        assert!(v.len() <= KEEP * 2, "{} linhas", v.len());
        assert_eq!(v.last().unwrap().summary, format!("n{}", KEEP * 2 + 29), "a mais nova nunca se perde");
        clear(&ctx).unwrap();
        assert!(!path(&ctx).exists());
        let _ = fs::remove_dir_all(&d);
    }
}
