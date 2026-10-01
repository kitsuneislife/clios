//! `clios shot`: captura de tela que salva, copia e avisa. Três comandos encadeados, um atalho.

use std::fs;
use std::path::PathBuf;
use std::process::{Command, Stdio};

use anyhow::{Context, Result, bail};

use crate::ctx::Ctx;

#[derive(Debug, Clone, Copy)]
pub enum Target {
    Region,
    Screen,
    Window,
}

fn out_dir() -> PathBuf {
    let base = std::env::var_os("XDG_PICTURES_DIR")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join("Pictures")))
        .unwrap_or_else(|| PathBuf::from("."));
    base.join("Screenshots")
}

fn timestamp() -> Result<String> {
    let out = Command::new("date").arg("+%Y%m%d-%H%M%S").output().context("rodando date")?;
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

fn geometry(target: Target, accent_hex: &str) -> Result<Option<String>> {
    match target {
        Target::Screen => Ok(None),
        Target::Region => {
            let out = Command::new("slurp")
                // fundo escurecido, borda no acento, 1px: a seleção parece da casa.
                .args(["-b", "#00000099", "-c", &format!("{accent_hex}ff"), "-s", "#00000000", "-w", "1"])
                .stderr(Stdio::null())
                .output()
                .context("slurp não encontrado (pacman -S slurp)")?;
            // slurp sai com 1 quando o usuário cancela com Esc: não é erro.
            if !out.status.success() {
                return Ok(Some(String::new()));
            }
            Ok(Some(String::from_utf8_lossy(&out.stdout).trim().to_string()))
        }
        Target::Window => {
            let out = Command::new("hyprctl").args(["activewindow", "-j"]).output().context("hyprctl")?;
            let v: serde_json::Value = serde_json::from_slice(&out.stdout).context("resposta do hyprctl")?;
            let (at, size) = (&v["at"], &v["size"]);
            let n = |x: &serde_json::Value| x.as_i64();
            match (n(&at[0]), n(&at[1]), n(&size[0]), n(&size[1])) {
                (Some(x), Some(y), Some(w), Some(h)) => Ok(Some(format!("{x},{y} {w}x{h}"))),
                _ => bail!("não há janela ativa"),
            }
        }
    }
}

pub fn run(ctx: &Ctx, target: Target) -> Result<()> {
    let theme = ctx.theme()?;
    let geo = geometry(target, &theme.c.accent.hex())?;
    if geo.as_deref() == Some("") {
        return Ok(()); // cancelado
    }

    let dir = out_dir();
    fs::create_dir_all(&dir).with_context(|| format!("criando {}", dir.display()))?;
    let file = dir.join(format!("clios-{}.png", timestamp()?));

    let mut grim = Command::new("grim");
    if let Some(g) = &geo {
        grim.args(["-g", g]);
    }
    let status = grim.arg(&file).status().context("grim não encontrado (pacman -S grim)")?;
    if !status.success() {
        bail!("grim falhou");
    }

    // Copia a imagem. Falha aqui não perde a captura, que já está em disco.
    if let Ok(bytes) = fs::read(&file) {
        use std::io::Write;
        if let Ok(mut c) = Command::new("wl-copy").args(["--type", "image/png"]).stdin(Stdio::piped()).spawn() {
            if let Some(mut stdin) = c.stdin.take() {
                let _ = stdin.write_all(&bytes);
            }
            let _ = c.wait();
        }
    }

    let _ = Command::new("notify-send")
        .args(["-a", "clios", "-i"])
        .arg(&file)
        .args(["captura", &format!("{}\ncopiada para a área de transferência", file.display())])
        .status();
    println!("{}", file.display());
    Ok(())
}
