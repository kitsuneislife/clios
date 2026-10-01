//! `clios self-update`: atualiza o próprio CLIOS. Puxa o repositório, recompila, reinstala o binário e roda o `sync`.
//!
//! O `clios update` cuida do Arch; este cuida do desktop em si. Fica separado de propósito: o Arch você atualiza
//! todo dia, o CLIOS quando quiser as novidades (CHANGELOG.md diz o que mudou).

use std::path::Path;
use std::process::{Command, Stdio};

use anyhow::{Result, bail};

use crate::ctx::Ctx;
use crate::ui;

/// Um passo: o texto que o usuário lê e o comando.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Step {
    pub what: &'static str,
    pub argv: Vec<String>,
}

fn s(v: &[&str]) -> Vec<String> {
    v.iter().map(ToString::to_string).collect()
}

/// O plano completo, na ordem. `root` é o checkout; `sudo` é `""` quando já somos root.
pub fn plan(root: &Path, sudo: bool) -> Vec<Step> {
    let r = root.display().to_string();
    let manifest = format!("{r}/Cargo.toml");
    let bin = format!("{r}/target/release/clios");
    let mut install = if sudo { s(&["sudo"]) } else { Vec::new() };
    install.extend(s(&["install", "-Dm755", &bin, "/usr/local/bin/clios"]));
    vec![
        Step { what: "puxando as novidades", argv: s(&["git", "-C", &r, "pull", "--ff-only"]) },
        Step {
            what: "compilando",
            argv: s(&["cargo", "build", "--release", "--locked", "--manifest-path", &manifest]),
        },
        Step { what: "instalando o binário", argv: install },
        // O binário novo é quem sincroniza: ele conhece os arquivos novos.
        Step { what: "ligando os arquivos e o tema", argv: s(&["/usr/local/bin/clios", "--root", &r, "sync"]) },
    ]
}

/// Quantos commits o repositório local está atrás do remoto (depois de um `fetch`). `None` se não deu para saber.
pub fn commits_behind(root: &Path) -> Option<usize> {
    let git = |args: &[&str]| {
        Command::new("git")
            .arg("-C")
            .arg(root)
            .args(args)
            .stderr(Stdio::null())
            .output()
            .ok()
            .filter(|o| o.status.success())
    };
    git(&["fetch", "--quiet"])?;
    let out = git(&["rev-list", "--count", "HEAD..@{u}"])?;
    String::from_utf8_lossy(&out.stdout).trim().parse().ok()
}

pub fn run(ctx: &Ctx, check: bool) -> Result<bool> {
    let root = &ctx.paths.root;
    if !root.join(".git").exists() {
        bail!(
            "{} não é um clone do git; o self-update só funciona num checkout (git clone + ./scripts/bootstrap.sh)",
            root.display()
        );
    }
    println!("{} {}", ui::bold("CLIOS"), ui::dim(&root.display().to_string()));
    match commits_behind(root) {
        Some(0) => {
            println!("já está na última versão");
            return Ok(true);
        }
        Some(n) => println!("{n} novidade(s) no repositório"),
        None => println!("{}", ui::dim("não consegui consultar o repositório remoto; tentando mesmo assim")),
    }
    if check {
        println!("rode `clios self-update` para atualizar");
        return Ok(true);
    }
    let sudo = !is_root();
    for (i, step) in plan(root, sudo).iter().enumerate() {
        println!("\n{} {}", ui::dim(&format!("{}.", i + 1)), ui::bold(step.what));
        let ok = Command::new(&step.argv[0]).args(&step.argv[1..]).status().is_ok_and(|s| s.success());
        if !ok {
            println!("\n{}", ui::dim("parou aqui. Nada foi desfeito; corrija e rode de novo."));
            return Ok(false);
        }
    }
    println!("\natualizado. As janelas abertas continuam com o binário antigo até fechar; o resto já é novo.");
    Ok(true)
}

fn is_root() -> bool {
    std::fs::metadata("/proc/self").map(|m| std::os::unix::fs::MetadataExt::uid(&m) == 0).unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_plan_pulls_builds_installs_and_syncs_in_that_order() {
        let p = plan(Path::new("/home/u/clios"), true);
        let whats: Vec<&str> = p.iter().map(|s| s.what).collect();
        assert_eq!(
            whats,
            ["puxando as novidades", "compilando", "instalando o binário", "ligando os arquivos e o tema"]
        );
        assert_eq!(p[0].argv, ["git", "-C", "/home/u/clios", "pull", "--ff-only"]);
        assert!(p[1].argv.contains(&"--locked".to_string()), "a build usa o Cargo.lock do repositório");
        assert_eq!(&p[2].argv[..2], ["sudo", "install"]);
        assert_eq!(p[3].argv[0], "/usr/local/bin/clios", "o binário novo é quem sincroniza");
    }

    #[test]
    fn root_does_not_need_sudo_and_the_pull_never_merges() {
        let p = plan(Path::new("/r"), false);
        assert_eq!(p[2].argv[0], "install");
        assert!(p[0].argv.contains(&"--ff-only".to_string()), "nada de merge surpresa no seu checkout");
    }
}
