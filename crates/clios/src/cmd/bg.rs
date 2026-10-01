//! Processos de segundo plano que o `clios` liga e desliga (café, modo noturno, gravação).
//!
//! Cada um guarda o pid em `state/run/<nome>.pid`. "Está ligado" quer dizer que o pid ainda existe
//! **e** é o programa esperado: um pid reaproveitado depois de um reboot não conta.

use std::fs;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use anyhow::{Context, Result};

use crate::ctx::Ctx;

#[derive(Debug, Clone, Copy)]
pub struct Bg {
    pub name: &'static str,
    /// Trecho que a linha de comando do processo tem que conter.
    pub needle: &'static str,
}

/// O pid ainda vive e a linha de comando dele contém `needle`?
pub fn alive(proc_root: &Path, pid: u32, needle: &str) -> bool {
    fs::read(proc_root.join(pid.to_string()).join("cmdline"))
        .is_ok_and(|raw| String::from_utf8_lossy(&raw).replace('\0', " ").contains(needle))
}

impl Bg {
    pub fn pid_file(&self, ctx: &Ctx) -> PathBuf {
        ctx.paths.state.join("run").join(format!("{}.pid", self.name))
    }

    pub fn pid(&self, ctx: &Ctx) -> Option<u32> {
        self.pid_in(ctx, Path::new("/proc"))
    }

    fn pid_in(&self, ctx: &Ctx, proc_root: &Path) -> Option<u32> {
        let pid: u32 = fs::read_to_string(self.pid_file(ctx)).ok()?.trim().parse().ok()?;
        alive(proc_root, pid, self.needle).then_some(pid)
    }

    pub fn running(&self, ctx: &Ctx) -> bool {
        self.pid(ctx).is_some()
    }

    /// Sobe o programa solto do terminal e guarda o pid.
    pub fn start(&self, ctx: &Ctx, prog: &str, args: &[String]) -> Result<u32> {
        let child = Command::new(prog)
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .process_group(0)
            .spawn()
            .with_context(|| format!("não achei {prog} (instale com: paru -S {prog})"))?;
        let pid = child.id();
        let file = self.pid_file(ctx);
        if let Some(dir) = file.parent() {
            fs::create_dir_all(dir)?;
        }
        fs::write(&file, format!("{pid}\n"))?;
        // Logo depois do fork, a linha de comando ainda é a do `clios`; espera o exec do programa
        // (até ~1 s) para quem chama poder perguntar `running` em seguida.
        for _ in 0..50 {
            if alive(Path::new("/proc"), pid, self.needle) {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        Ok(pid)
    }

    /// Manda `signal` (`TERM`, `INT`) ao processo e espera ele sair (até ~3 s). `false` se não estava rodando.
    pub fn stop(&self, ctx: &Ctx, signal: &str) -> Result<bool> {
        let Some(pid) = self.pid(ctx) else {
            let _ = fs::remove_file(self.pid_file(ctx));
            return Ok(false);
        };
        Command::new("kill").arg(format!("-{signal}")).arg(pid.to_string()).status().context("rodando kill")?;
        for _ in 0..30 {
            if !alive(Path::new("/proc"), pid, self.needle) {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
        let _ = fs::remove_file(self.pid_file(ctx));
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn ctx(name: &str) -> Ctx {
        let home = std::env::temp_dir().join(format!("clios-bg-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&home);
        fs::create_dir_all(&home).unwrap();
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        Ctx::load(Some(&root), Some(&home)).unwrap()
    }

    fn fake_proc(ctx: &Ctx, pid: u32, cmdline: &[&str]) -> PathBuf {
        let root = ctx.paths.home.join("proc");
        let dir = root.join(pid.to_string());
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("cmdline"), cmdline.join("\0")).unwrap();
        root
    }

    #[test]
    fn alive_checks_the_command_line_not_just_the_pid() {
        let c = ctx("alive");
        let root = fake_proc(&c, 4242, &["systemd-inhibit", "--what=idle", "sleep", "infinity"]);
        assert!(alive(&root, 4242, "systemd-inhibit"));
        assert!(!alive(&root, 4242, "hyprsunset"), "pid reaproveitado por outro programa");
        assert!(!alive(&root, 9999, "systemd-inhibit"), "pid que não existe");
    }

    #[test]
    fn pid_file_roundtrip_and_stale_pids() {
        let c = ctx("pidfile");
        let bg = Bg { name: "caffeine", needle: "systemd-inhibit" };
        assert_eq!(bg.pid_in(&c, Path::new("/proc")), None, "sem arquivo, desligado");
        let root = fake_proc(&c, 777, &["systemd-inhibit"]);
        fs::create_dir_all(bg.pid_file(&c).parent().unwrap()).unwrap();
        fs::write(bg.pid_file(&c), "777\n").unwrap();
        assert_eq!(bg.pid_in(&c, &root), Some(777));
        fs::write(bg.pid_file(&c), "888\n").unwrap();
        assert_eq!(bg.pid_in(&c, &root), None, "o pid do arquivo morreu");
        fs::write(bg.pid_file(&c), "lixo").unwrap();
        assert_eq!(bg.pid_in(&c, &root), None);
    }

    #[test]
    fn start_and_stop_a_real_process() {
        let c = ctx("real");
        let bg = Bg { name: "sleeper", needle: "sleep" };
        let pid = bg.start(&c, "sleep", &["30".into()]).unwrap();
        assert!(bg.running(&c));
        assert_eq!(bg.pid(&c), Some(pid));
        assert!(bg.stop(&c, "TERM").unwrap());
        assert!(!bg.running(&c));
        assert!(!bg.stop(&c, "TERM").unwrap(), "parar de novo não faz nada");
    }

    #[test]
    fn starting_a_missing_program_explains_how_to_install_it() {
        let c = ctx("missing");
        let bg = Bg { name: "x", needle: "x" };
        let err = bg.start(&c, "clios-programa-que-nao-existe", &[]).unwrap_err().to_string();
        assert!(err.contains("paru -S"), "{err}");
    }
}
