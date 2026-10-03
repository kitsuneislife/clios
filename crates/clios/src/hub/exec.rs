//! Transforma uma ação do hub em um comando. O plano é puro (e testável); só `spawn` toca o sistema.

use std::process::{Command, Stdio};

use anyhow::{Context, Result, bail};

use super::items::Action;
use crate::sys::sh_quote;

pub struct Env {
    /// Caminho deste executável, para as ações que reexecutam o clios.
    pub exe: String,
    /// Há um `foot --server` rodando? Então `footclient` abre janela instantânea.
    pub foot_server: bool,
}

impl Env {
    pub fn detect() -> Self {
        let exe = std::env::current_exe().map_or_else(|_| "clios".into(), |p| p.to_string_lossy().into_owned());
        Self { exe, foot_server: foot_server_running() }
    }
}

fn foot_server_running() -> bool {
    let Some(rt) = std::env::var_os("XDG_RUNTIME_DIR") else { return false };
    let display = std::env::var("WAYLAND_DISPLAY").unwrap_or_default();
    let name = if display.is_empty() { "foot.sock".to_string() } else { format!("foot-{display}.sock") };
    std::path::Path::new(&rt).join(name).exists()
}

/// Tamanho inicial (em células) das janelas flutuantes.
pub const FLOAT_SIZE: &str = "104x32";

/// `None` para ações que não rodam nada (informativas ou internas do hub).
pub fn plan(action: &Action, env: &Env) -> Option<Vec<String>> {
    let detached = |rest: Vec<String>| -> Option<Vec<String>> {
        let mut v = vec!["setsid".to_string(), "-f".to_string()];
        v.extend(rest);
        Some(v)
    };
    let s = |x: &str| x.to_string();

    match action {
        Action::None | Action::Scope(_) => None,
        Action::Gui(cmd) | Action::Shell(cmd) => detached(vec![s("sh"), s("-c"), cmd.clone()]),
        Action::Hypr(expr) => detached(vec![s("hyprctl"), s("dispatch"), expr.clone()]),
        Action::Clios(args) => {
            let mut v = vec![env.exe.clone()];
            v.extend(args.iter().cloned());
            detached(v)
        }
        Action::Copy(text) => detached(vec![
            s("sh"),
            s("-c"),
            s("printf %s \"$1\" | wl-copy && notify-send -a clios -u low copiado \"$1\""),
            s("sh"),
            text.clone(),
        ]),
        Action::Clip(id) => {
            detached(vec![s("sh"), s("-c"), s("cliphist decode \"$1\" | wl-copy"), s("sh"), id.clone()])
        }
        Action::Tui { id, argv, float, hold } => {
            let mut cmd: Vec<String> = argv.clone();
            if cmd.first().is_some_and(|c| c == "clios") {
                cmd[0] = env.exe.clone();
            }
            if *hold {
                let line = cmd.iter().map(|a| sh_quote(a)).collect::<Vec<_>>().join(" ");
                cmd = vec![s("sh"), s("-c"), format!("{line}; printf '\\n\\033[2m↵ fechar\\033[0m'; read _")];
            }
            let mut v = vec![s(if env.foot_server { "footclient" } else { "foot" })];
            let class = if *float { format!("clios.float.{id}") } else { format!("clios.tui.{id}") };
            v.extend([s("-a"), class]);
            if *float {
                v.extend([s("-W"), s(FLOAT_SIZE)]);
            }
            v.extend(cmd);
            detached(v)
        }
    }
}

/// A janela do próprio hub: flutuante, centralizada pela regra do Hyprland, tamanho fixo em células.
pub const HUB_SIZE: &str = "84x24";

pub fn plan_hub(env: &Env, query: &str) -> Vec<String> {
    let mut v = vec!["setsid".to_string(), "-f".to_string()];
    v.push(if env.foot_server { "footclient" } else { "foot" }.to_string());
    v.extend(["-a", "clios.hub", "-W", HUB_SIZE].map(String::from));
    v.extend([env.exe.clone(), "hub".to_string()]);
    if !query.is_empty() {
        v.extend(["--query".to_string(), query.to_string()]);
    }
    v
}

pub fn spawn(argv: &[String]) -> Result<()> {
    let Some((prog, args)) = argv.split_first() else { bail!("comando vazio") };
    let status = Command::new(prog)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .with_context(|| format!("não consegui executar {prog}"))?;
    if !status.success() {
        bail!("{prog} saiu com {status}");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hub::items::Scope;

    fn env(server: bool) -> Env {
        Env { exe: "/usr/bin/clios".into(), foot_server: server }
    }

    #[test]
    fn floating_tui_uses_footclient_with_class_and_size() {
        let a = Action::Tui { id: "audio".into(), argv: vec!["wiremix".into()], float: true, hold: false };
        assert_eq!(
            plan(&a, &env(true)).unwrap(),
            ["setsid", "-f", "footclient", "-a", "clios.float.audio", "-W", FLOAT_SIZE, "wiremix"]
        );
    }

    #[test]
    fn tiled_tui_has_no_size_and_falls_back_to_foot_without_server() {
        let a = Action::Tui { id: "files".into(), argv: vec!["yazi".into()], float: false, hold: false };
        assert_eq!(plan(&a, &env(false)).unwrap(), ["setsid", "-f", "foot", "-a", "clios.tui.files", "yazi"]);
    }

    #[test]
    fn hold_wraps_the_command_and_quotes_arguments() {
        let a = Action::Tui {
            id: "doctor".into(),
            argv: vec!["clios".into(), "doctor".into(), "a b".into()],
            float: true,
            hold: true,
        };
        let p = plan(&a, &env(true)).unwrap();
        assert_eq!(&p[p.len() - 3..p.len() - 1], ["sh", "-c"]);
        let script = p.last().unwrap();
        assert!(script.starts_with("/usr/bin/clios doctor 'a b'; printf"), "{script}");
        assert!(script.ends_with("read _"));
    }

    #[test]
    fn gui_and_shell_run_through_sh() {
        assert_eq!(plan(&Action::Gui("firefox".into()), &env(true)).unwrap(), ["setsid", "-f", "sh", "-c", "firefox"]);
        assert_eq!(plan(&Action::Shell("systemctl suspend".into()), &env(true)).unwrap()[4], "systemctl suspend");
    }

    #[test]
    fn clios_actions_reexec_this_binary() {
        let a = Action::Clios(vec!["theme".into(), "toggle".into()]);
        assert_eq!(plan(&a, &env(true)).unwrap(), ["setsid", "-f", "/usr/bin/clios", "theme", "toggle"]);
    }

    #[test]
    fn clip_id_is_passed_as_an_argument_never_interpolated() {
        let p = plan(&Action::Clip("12; rm -rf ~".into()), &env(true)).unwrap();
        assert_eq!(p[4], "cliphist decode \"$1\" | wl-copy");
        assert_eq!(p[6], "12; rm -rf ~");
    }

    #[test]
    fn hyprland_dispatch() {
        let p = plan(&Action::Hypr("hl.dsp.exit()".into()), &env(true)).unwrap();
        assert_eq!(p, ["setsid", "-f", "hyprctl", "dispatch", "hl.dsp.exit()"]);
    }

    #[test]
    fn hub_window_has_its_own_class_and_size() {
        assert_eq!(
            plan_hub(&env(true), ""),
            ["setsid", "-f", "footclient", "-a", "clios.hub", "-W", HUB_SIZE, "/usr/bin/clios", "hub"]
        );
        let q = plan_hub(&env(false), ">");
        assert_eq!(&q[q.len() - 4..], ["/usr/bin/clios", "hub", "--query", ">"]);
        assert_eq!(q[2], "foot");
    }

    #[test]
    fn internal_actions_run_nothing() {
        assert!(plan(&Action::None, &env(true)).is_none());
        assert!(plan(&Action::Scope(Scope::Keys), &env(true)).is_none());
    }
}
