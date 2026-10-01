//! Busca difusa (nucleo, o mesmo motor do helix) com histórico de uso.

use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{Context, Result};
use clios_core::fsutil::write_atomic;
use nucleo_matcher::pattern::{CaseMatching, Normalization, Pattern};
use nucleo_matcher::{Config, Matcher, Utf32Str};
use serde::{Deserialize, Serialize};

use super::items::{Item, Kind, Scope};

const MAX_HISTORY: usize = 400;

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct History {
    #[serde(default)]
    entries: BTreeMap<String, Entry>,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
struct Entry {
    count: u32,
    /// Segundos Unix do último uso.
    last: u64,
}

impl History {
    pub fn load(path: &Path) -> Self {
        // Histórico é conveniência: arquivo ilegível vira histórico vazio, nunca um erro no hub.
        std::fs::read_to_string(path).ok().and_then(|s| toml::from_str(&s).ok()).unwrap_or_default()
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        let body = toml::to_string(self).context("serializando histórico")?;
        write_atomic(path, body.as_bytes())
    }

    pub fn record(&mut self, id: &str, now: u64) {
        let e = self.entries.entry(id.to_string()).or_insert(Entry { count: 0, last: now });
        e.count = e.count.saturating_add(1);
        e.last = now;
        if self.entries.len() > MAX_HISTORY {
            // Descarta o menos usado e mais antigo.
            if let Some(victim) = self.entries.iter().min_by_key(|(_, e)| (e.count, e.last)).map(|(k, _)| k.clone()) {
                self.entries.remove(&victim);
            }
        }
    }

    pub fn used(&self, id: &str) -> bool {
        self.entries.contains_key(id)
    }

    /// Bônus de "frecency": frequência amortecida pelo tempo desde o último uso.
    pub fn bonus(&self, id: &str, now: u64) -> u32 {
        let Some(e) = self.entries.get(id) else { return 0 };
        let days = now.saturating_sub(e.last) as f64 / 86_400.0;
        let freq = (1.0 + f64::from(e.count)).ln() * 25.0;
        (freq / (1.0 + days / 14.0)) as u32
    }
}

#[derive(Debug, Clone)]
pub struct Hit {
    /// Posição em `items`.
    pub index: usize,
    pub score: u32,
    /// Caracteres do título que casaram, para destacar.
    pub title_idx: Vec<u32>,
}

pub struct Ranker {
    matcher: Matcher,
    buf: Vec<char>,
}

impl Default for Ranker {
    fn default() -> Self {
        Self::new()
    }
}

impl Ranker {
    pub fn new() -> Self {
        Self { matcher: Matcher::new(Config::DEFAULT), buf: Vec::new() }
    }

    pub fn rank(&mut self, items: &[Item], scope: Scope, query: &str, history: &History, now: u64) -> Vec<Hit> {
        let query = query.trim();
        let mut hits = Vec::new();

        if query.is_empty() {
            for (index, item) in items.iter().enumerate() {
                if !scope.includes(item.kind) {
                    continue;
                }
                // Na tela inicial, ação que você nunca usou é ruído.
                if scope == Scope::All && item.kind == Kind::Action && !history.used(&item.id) {
                    continue;
                }
                let score = history.bonus(&item.id, now);
                hits.push(Hit { index, score, title_idx: Vec::new() });
            }
            // Estável: quem tem o mesmo bônus mantém a ordem original.
            hits.sort_by_key(|h| std::cmp::Reverse(h.score));
            return hits;
        }

        let pattern = Pattern::parse(query, CaseMatching::Smart, Normalization::Smart);
        let mut idx = Vec::new();
        for (index, item) in items.iter().enumerate() {
            if !scope.includes(item.kind) {
                continue;
            }
            idx.clear();
            let title = Utf32Str::new(&item.title, &mut self.buf);
            let base = if let Some(s) = pattern.indices(title, &mut self.matcher, &mut idx) {
                idx.sort_unstable();
                idx.dedup();
                Some((s * 3, idx.clone()))
            } else if !item.keywords.is_empty() {
                let hay = format!("{} {}", item.title, item.keywords);
                let hay = Utf32Str::new(&hay, &mut self.buf);
                pattern.score(hay, &mut self.matcher).map(|s| (s, Vec::new()))
            } else {
                None
            };
            if let Some((score, title_idx)) = base {
                hits.push(Hit { index, score: score + history.bonus(&item.id, now), title_idx });
            }
        }
        hits.sort_by(|a, b| b.score.cmp(&a.score).then(items[a.index].title.len().cmp(&items[b.index].title.len())));
        hits
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hub::items::Action;

    fn item(kind: Kind, id: &str, title: &str, kw: &str) -> Item {
        Item::new(kind, id, title, Action::None).keywords(kw)
    }

    fn titles(items: &[Item], hits: &[Hit]) -> Vec<String> {
        hits.iter().map(|h| items[h.index].title.clone()).collect()
    }

    fn run(items: &[Item], scope: Scope, q: &str, h: &History) -> Vec<Hit> {
        Ranker::new().rank(items, scope, q, h, 1_000_000)
    }

    #[test]
    fn subsequence_prefers_tighter_match() {
        let items = vec![item(Kind::App, "a", "Office Tool", ""), item(Kind::App, "b", "Firefox", "")];
        let hits = run(&items, Scope::All, "ff", &History::default());
        assert_eq!(titles(&items, &hits)[0], "Firefox");
    }

    #[test]
    fn highlights_matched_title_chars() {
        let items = vec![item(Kind::App, "b", "Firefox", "")];
        let hits = run(&items, Scope::All, "fire", &History::default());
        assert_eq!(hits[0].title_idx, vec![0, 1, 2, 3]);
    }

    #[test]
    fn highlight_indices_are_sorted_unique_and_inside_the_title() {
        // Qual alinhamento o nucleo escolhe é detalhe dele; o contrato é este.
        let items = vec![item(Kind::App, "b", "Firefox Developer Edition", "")];
        for q in ["fox", "fde", "dev ed", "ff", "x"] {
            let hits = run(&items, Scope::All, q, &History::default());
            let idx = &hits[0].title_idx;
            assert!(!idx.is_empty(), "{q}");
            assert!(idx.windows(2).all(|w| w[0] < w[1]), "{q}: {idx:?}");
            assert!(idx.iter().all(|i| (*i as usize) < items[0].title.chars().count()), "{q}");
        }
    }

    #[test]
    fn keywords_match_without_highlighting() {
        let items = vec![item(Kind::Tui, "net", "rede", "wifi impala wireless")];
        let hits = run(&items, Scope::All, "wifi", &History::default());
        assert_eq!(hits.len(), 1);
        assert!(hits[0].title_idx.is_empty());
    }

    #[test]
    fn no_match_means_no_hit() {
        let items = vec![item(Kind::App, "a", "Firefox", "")];
        assert!(run(&items, Scope::All, "zzz", &History::default()).is_empty());
    }

    #[test]
    fn smart_case_uppercase_is_strict() {
        let items = vec![item(Kind::App, "a", "firefox", "")];
        assert_eq!(run(&items, Scope::All, "fire", &History::default()).len(), 1);
        assert!(run(&items, Scope::All, "Fire", &History::default()).is_empty());
    }

    #[test]
    fn scope_filters_by_kind() {
        let items = vec![
            item(Kind::App, "a", "foo app", ""),
            item(Kind::Window, "w", "foo window", ""),
            item(Kind::Key, "k", "foo key", ""),
        ];
        let h = History::default();
        assert_eq!(titles(&items, &run(&items, Scope::All, "foo", &h)), vec!["foo app"]);
        assert_eq!(titles(&items, &run(&items, Scope::Windows, "foo", &h)), vec!["foo window"]);
        assert_eq!(titles(&items, &run(&items, Scope::Keys, "foo", &h)), vec!["foo key"]);
    }

    #[test]
    fn empty_query_hides_unused_actions_but_shows_used_ones() {
        let items = vec![
            item(Kind::Tui, "t", "arquivos", ""),
            item(Kind::Action, "x", "reiniciar", ""),
            item(Kind::Action, "y", "bloquear", ""),
        ];
        let mut h = History::default();
        assert_eq!(titles(&items, &run(&items, Scope::All, "", &h)), vec!["arquivos"]);
        h.record("y", 1_000_000);
        assert_eq!(titles(&items, &run(&items, Scope::All, "", &h)), vec!["bloquear", "arquivos"]);
        // No escopo de ações, tudo aparece.
        assert_eq!(run(&items, Scope::Actions, "", &h).len(), 2);
    }

    #[test]
    fn history_breaks_ties() {
        let items = vec![item(Kind::App, "a", "foo", ""), item(Kind::App, "b", "foo", "")];
        let mut h = History::default();
        h.record("b", 1_000_000);
        h.record("b", 1_000_000);
        let hits = run(&items, Scope::All, "foo", &h);
        assert_eq!(items[hits[0].index].id, "b");
    }

    #[test]
    fn bonus_decays_with_time() {
        let mut h = History::default();
        for _ in 0..5 {
            h.record("a", 0);
        }
        let fresh = h.bonus("a", 0);
        let month = h.bonus("a", 30 * 86_400);
        assert!(fresh > month && month > 0, "{fresh} {month}");
        assert_eq!(h.bonus("desconhecido", 0), 0);
    }

    #[test]
    fn history_is_bounded() {
        let mut h = History::default();
        for i in 0..(MAX_HISTORY + 50) {
            h.record(&format!("id{i}"), i as u64);
        }
        assert!(h.entries.len() <= MAX_HISTORY);
    }

    #[test]
    fn history_roundtrip_and_corrupt_file_is_empty() {
        let d = std::env::temp_dir().join(format!("clios-hist-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        let f = d.join("h.toml");
        let mut h = History::default();
        h.record("app:x", 42);
        h.save(&f).unwrap();
        assert!(History::load(&f).used("app:x"));
        std::fs::write(&f, "isto não é toml [[[").unwrap();
        assert!(!History::load(&f).used("app:x"));
        assert!(!History::load(&d.join("não-existe.toml")).used("app:x"));
    }
}
