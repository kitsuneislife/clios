//! `clios doctor`: o que falta para o desktop funcionar por inteiro.

use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::Result;

use crate::ctx::Ctx;
use crate::ui;

struct Check {
    bin: &'static str,
    why: &'static str,
}

const REQUIRED: &[Check] = &[
    Check { bin: "Hyprland", why: "o compositor" },
    Check { bin: "hyprctl", why: "controle do compositor (hub, janelas, atalhos)" },
    Check { bin: "quickshell", why: "a shell inteira: barra, notificações, OSD, papel de parede" },
    Check { bin: "foot", why: "o terminal" },
    Check { bin: "footclient", why: "janelas de terminal instantâneas" },
    Check { bin: "fish", why: "o shell" },
    Check { bin: "hx", why: "o editor (helix)" },
    Check { bin: "yazi", why: "arquivos" },
    Check { bin: "setsid", why: "abrir apps desacoplados do hub" },
];

const RECOMMENDED: &[Check] = &[
    Check { bin: "hyprlock", why: "tela de bloqueio" },
    Check { bin: "hypridle", why: "bloquear e suspender sozinho" },
    Check { bin: "lazygit", why: "git" },
    Check { bin: "btop", why: "monitor do sistema" },
    Check { bin: "wiremix", why: "áudio" },
    Check { bin: "impala", why: "wi-fi" },
    Check { bin: "bluetui", why: "bluetooth" },
    Check { bin: "spotify_player", why: "música" },
    Check { bin: "mpv", why: "vídeo e áudio" },
    Check { bin: "grim", why: "capturas de tela" },
    Check { bin: "slurp", why: "seleção de região" },
    Check { bin: "wl-copy", why: "área de transferência (wl-clipboard)" },
    Check { bin: "cliphist", why: "histórico da área de transferência" },
    Check { bin: "notify-send", why: "notificações a partir de scripts" },
    Check { bin: "brightnessctl", why: "brilho da tela" },
    Check { bin: "playerctl", why: "teclas de mídia" },
    Check { bin: "starship", why: "prompt" },
    Check { bin: "fzf", why: "busca difusa no shell" },
    Check { bin: "rg", why: "ripgrep" },
    Check { bin: "fd", why: "busca de arquivos" },
    Check { bin: "bat", why: "cat com cor" },
    Check { bin: "eza", why: "ls" },
    Check { bin: "zoxide", why: "cd inteligente" },
    Check { bin: "delta", why: "diffs legíveis no git" },
    Check { bin: "jq", why: "JSON" },
];

const OPTIONAL: &[Check] = &[
    Check { bin: "ani-cli", why: "anime (AUR)" },
    Check { bin: "aerc", why: "e-mail" },
    Check { bin: "newsboat", why: "feeds RSS" },
    Check { bin: "firefox", why: "a única exceção gráfica: o navegador" },
    Check { bin: "zathura", why: "PDF" },
    Check { bin: "imv", why: "imagens" },
];

fn find_in_path(bin: &str) -> Option<PathBuf> {
    let path = env::var_os("PATH")?;
    env::split_paths(&path).map(|d| d.join(bin)).find(|p| is_executable(p))
}

fn is_executable(p: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    p.metadata().is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
}

fn has_font(family: &str) -> Option<bool> {
    let out = Command::new("fc-list").arg(":").arg("family").output().ok()?;
    Some(String::from_utf8_lossy(&out.stdout).to_lowercase().contains(&family.to_lowercase()))
}

/// Devolve `true` se tudo que é obrigatório está presente.
pub fn run(ctx: &Ctx) -> Result<bool> {
    let mut missing_required = 0;

    println!("{}", ui::bold("obrigatórios"));
    for c in REQUIRED {
        if !report(c, true) {
            missing_required += 1;
        }
    }
    println!("\n{}", ui::bold("recomendados"));
    for c in RECOMMENDED {
        report(c, false);
    }
    println!("\n{}", ui::bold("opcionais"));
    for c in OPTIONAL {
        report(c, false);
    }

    println!("\n{}", ui::bold("sistema"));
    match has_font("GeistMono") {
        Some(true) => println!("  ✓ fonte GeistMono Nerd Font"),
        Some(false) => println!("  ✗ fonte GeistMono Nerd Font  {}", ui::dim("pacman -S otf-geist-mono-nerd")),
        None => println!("  · fonte: fc-list indisponível, não consegui checar"),
    }
    let theme_json = ctx.paths.state.join("theme.json");
    if theme_json.is_file() {
        println!("  ✓ tema aplicado ({})", theme_json.display());
    } else {
        println!("  ✗ tema ainda não aplicado  {}", ui::dim("rode: clios sync"));
    }
    let hypr = ctx.paths.config.join("hypr/hyprland.lua");
    if hypr.is_file() {
        println!("  ✓ {}", hypr.display());
    } else {
        println!("  ✗ falta {}  {}", hypr.display(), ui::dim("rode: clios sync"));
    }

    if missing_required == 0 {
        println!("\ntudo que é obrigatório está no lugar.");
    } else {
        println!("\n{missing_required} item(ns) obrigatório(s) faltando.");
    }
    Ok(missing_required == 0)
}

fn report(c: &Check, required: bool) -> bool {
    match find_in_path(c.bin) {
        Some(p) => {
            println!("  ✓ {:<15} {}", c.bin, ui::dim(&p.display().to_string()));
            true
        }
        None => {
            let mark = if required { "✗" } else { "·" };
            println!("  {mark} {:<15} {}", c.bin, ui::dim(&format!("falta: {}", c.why)));
            false
        }
    }
}
