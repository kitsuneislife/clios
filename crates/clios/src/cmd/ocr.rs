//! `clios ocr`: seleciona uma região da tela e copia o texto que está nela. Serve para o que não se deixa
//! selecionar: imagens, vídeos, PDFs escaneados, janelas de apps gráficos. Usa o tesseract.

use std::io::Write;
use std::process::{Command, Stdio};

use anyhow::{Context, Result, bail};

use super::shot::{self, Target};
use crate::ctx::Ctx;

/// Escolhe os idiomas do tesseract a partir de `tesseract --list-langs`: português e inglês, os que estiverem.
pub fn pick_langs(list: &str) -> Option<String> {
    let have: Vec<&str> = list.lines().map(str::trim).collect();
    let langs: Vec<&str> = ["por", "eng"].into_iter().filter(|l| have.contains(l)).collect();
    (!langs.is_empty()).then(|| langs.join("+"))
}

/// Tira o que o OCR costuma deixar de sobra: espaços no fim das linhas e linhas vazias nas pontas.
pub fn tidy(text: &str) -> String {
    let lines: Vec<&str> = text.lines().map(str::trim_end).collect();
    lines.join("\n").trim_matches('\n').to_string()
}

fn notify(body: &str) {
    let _ = Command::new("notify-send").args(["-a", "clios", "texto copiado", body]).status();
}

pub fn run(ctx: &Ctx) -> Result<()> {
    let list = Command::new("tesseract")
        .arg("--list-langs")
        .output()
        .context("tesseract não encontrado (pacman -S tesseract tesseract-data-por tesseract-data-eng)")?;
    // `--list-langs` escreve na saída padrão nas versões novas e na de erro nas antigas.
    let both = format!("{}{}", String::from_utf8_lossy(&list.stdout), String::from_utf8_lossy(&list.stderr));
    let Some(langs) = pick_langs(&both) else {
        bail!("faltam os dados de idioma do tesseract (pacman -S tesseract-data-por tesseract-data-eng)");
    };

    let theme = ctx.theme()?;
    let geo = shot::geometry(Target::Region, &theme.c.accent.hex())?;
    let Some(geo) = geo.filter(|g| !g.is_empty()) else { return Ok(()) };

    let png = Command::new("grim")
        .args(["-g", &geo, "-t", "png", "-"])
        .stderr(Stdio::null())
        .output()
        .context("grim não encontrado (pacman -S grim)")?;
    if !png.status.success() {
        bail!("grim falhou");
    }

    let mut ocr = Command::new("tesseract")
        .args(["stdin", "stdout", "-l", &langs, "--psm", "6"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .context("rodando o tesseract")?;
    ocr.stdin.take().context("stdin do tesseract")?.write_all(&png.stdout)?;
    let out = ocr.wait_with_output()?;
    let text = tidy(&String::from_utf8_lossy(&out.stdout));
    if text.is_empty() {
        notify("não achei texto nessa região");
        return Ok(());
    }

    let mut copy = Command::new("wl-copy")
        .stdin(Stdio::piped())
        .spawn()
        .context("wl-copy não encontrado (pacman -S wl-clipboard)")?;
    copy.stdin.take().context("stdin do wl-copy")?.write_all(text.as_bytes())?;
    copy.wait()?;
    let preview: String = text.lines().next().unwrap_or("").chars().take(80).collect();
    notify(&preview);
    println!("{text}");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn languages_prefer_portuguese_plus_english_and_use_what_is_there() {
        let both = "List of available languages in \"/usr/share/tessdata/\" (3):\neng\nosd\npor\n";
        assert_eq!(pick_langs(both).as_deref(), Some("por+eng"));
        assert_eq!(pick_langs("List of available languages (1):\neng\n").as_deref(), Some("eng"));
        assert_eq!(pick_langs("List of available languages (1):\nosd\n"), None, "osd não lê texto");
    }

    #[test]
    fn tidy_strips_trailing_space_and_blank_edges() {
        assert_eq!(tidy("\n\nolá  \nmundo\t\n\n"), "olá\nmundo");
        assert_eq!(tidy("   \n"), "");
    }
}
