//! Pequenas consultas ao sistema, compartilhadas pelos comandos.

use std::env;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

pub fn is_executable(p: &Path) -> bool {
    p.metadata().is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
}

pub fn find_in_path(bin: &str) -> Option<PathBuf> {
    let path = env::var_os("PATH")?;
    env::split_paths(&path).map(|d| d.join(bin)).find(|p| is_executable(p))
}

pub fn is_installed(bin: &str) -> bool {
    find_in_path(bin).is_some()
}

/// Aspas simples POSIX.
pub fn sh_quote(s: &str) -> String {
    if !s.is_empty() && s.chars().all(|c| c.is_ascii_alphanumeric() || "-_./=:,@%+".contains(c)) {
        return s.to_string();
    }
    format!("'{}'", s.replace('\'', "'\\''"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quoting() {
        assert_eq!(sh_quote("abc-1.2"), "abc-1.2");
        assert_eq!(sh_quote("a b"), "'a b'");
        assert_eq!(sh_quote("it's"), "'it'\\''s'");
        assert_eq!(sh_quote(""), "''");
        assert_eq!(sh_quote("$(rm -rf)"), "'$(rm -rf)'");
    }

    #[test]
    fn sh_is_always_installed_and_nonsense_is_not() {
        assert!(is_installed("sh"));
        assert!(!is_installed("isto-nao-existe-clios-test"));
    }
}
