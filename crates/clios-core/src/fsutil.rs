//! Escrita de arquivos sem deixar ninguém ver meio-arquivo.

use std::fs;
use std::io::Write;
use std::path::Path;

use anyhow::{Context, Result};

/// Escreve em arquivo temporário vizinho e renomeia por cima. Cria diretórios que faltarem.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    let dir = path.parent().with_context(|| format!("{} não tem diretório pai", path.display()))?;
    fs::create_dir_all(dir).with_context(|| format!("criando {}", dir.display()))?;
    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let tmp = dir.join(format!(".{name}.clios-tmp-{}", std::process::id()));
    {
        let mut f = fs::File::create(&tmp).with_context(|| format!("criando {}", tmp.display()))?;
        f.write_all(bytes)?;
        f.sync_all()?;
    }
    fs::rename(&tmp, path).with_context(|| format!("renomeando para {}", path.display()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_and_replaces_without_leaving_temp_files() {
        let d = std::env::temp_dir().join(format!("clios-fs-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        let f = d.join("x/y.txt");
        write_atomic(&f, b"um").unwrap();
        write_atomic(&f, b"dois").unwrap();
        assert_eq!(fs::read_to_string(&f).unwrap(), "dois");
        let leftovers: Vec<_> =
            fs::read_dir(d.join("x")).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
        assert_eq!(leftovers, vec!["y.txt"]);
    }
}
