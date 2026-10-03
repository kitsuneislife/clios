//! O diálogo de abrir e salvar arquivo, no terminal.
//!
//! Quando o Firefox (ou qualquer app que fale com o portal) pede um arquivo, o xdg-desktop-portal-termfilechooser chama
//! `clios-filechooser` (um link para este binário) com cinco argumentos. Aqui abrimos uma janela flutuante com o yazi
//! para escolher, e um campo de nome para salvar. O resultado vai para o arquivo de saída, um caminho por linha; nada
//! escrito quer dizer cancelado.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, bail};
use ratatui::crossterm::{
    cursor,
    event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers},
    execute, terminal,
};

use crate::sys::{is_installed, sh_quote};

/// O que o app pediu.
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum Mode {
    Open,
    Multi,
    Dir,
    Save,
}

/// Os argumentos do portal: `multiple directory save path out [debug]`.
pub fn parse_portal(args: &[String]) -> Result<(Mode, PathBuf, PathBuf)> {
    if args.len() < 5 {
        bail!("uso: clios-filechooser <multiple> <directory> <save> <caminho> <saída> (quem chama é o portal)");
    }
    let on = |s: &str| s == "1";
    let mode = match (on(&args[0]), on(&args[1]), on(&args[2])) {
        (_, _, true) => Mode::Save,
        (_, true, _) => Mode::Dir,
        (true, _, _) => Mode::Multi,
        _ => Mode::Open,
    };
    Ok((mode, PathBuf::from(&args[3]), PathBuf::from(&args[4])))
}

fn title(mode: Mode) -> &'static str {
    match mode {
        Mode::Open => "abrir arquivo",
        Mode::Multi => "abrir arquivos",
        Mode::Dir => "escolher pasta",
        Mode::Save => "salvar como",
    }
}

/// A janela: flutuante (a regra `clios.float.*` centraliza), com o diálogo dentro.
pub fn window_argv(exe: &str, server: bool, mode: Mode, path: &Path, out: &Path) -> Vec<String> {
    let mut v: Vec<String> = if server { vec!["footclient".into()] } else { vec!["foot".into()] };
    v.extend([
        "--app-id=clios.float.picker".to_string(),
        "--window-size-chars=110x30".to_string(),
        format!("--title={}", title(mode)),
        exe.to_string(),
        "filechooser".to_string(),
        "--mode".to_string(),
        format!("{mode:?}").to_lowercase(),
        "--path".to_string(),
        path.display().to_string(),
        "--out".to_string(),
        out.display().to_string(),
    ]);
    v
}

fn foot_server_running() -> bool {
    let Some(rt) = std::env::var_os("XDG_RUNTIME_DIR") else { return false };
    let display = std::env::var("WAYLAND_DISPLAY").unwrap_or_default();
    let name = if display.is_empty() { "foot.sock".to_string() } else { format!("foot-{display}.sock") };
    Path::new(&rt).join(name).exists()
}

/// Entrada do portal: abre a janela e espera ela fechar.
pub fn portal(args: &[String]) -> Result<()> {
    let (mode, path, out) = parse_portal(args)?;
    let exe = std::env::current_exe().map_or_else(|_| "clios".into(), |p| p.display().to_string());
    let argv = window_argv(&exe, foot_server_running(), mode, &path, &out);
    let st = Command::new(&argv[0]).args(&argv[1..]).status().with_context(|| format!("abrindo {}", argv[0]))?;
    if !st.success() {
        bail!("{} saiu com {st}", argv[0]);
    }
    Ok(())
}

// ── o campo de nome ───────────────────────────────────────────────────────

/// Um campo de texto de uma linha, com os atalhos do shell.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Field {
    pub text: String,
    /// Posição do cursor, em caracteres.
    pub cursor: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Editing,
    Submit,
    Browse,
    Cancel,
}

impl Field {
    pub fn new(text: &str) -> Self {
        // o cursor para antes da extensão: renomear é o caso comum, trocar a extensão não
        let stem = Path::new(text).file_stem().map_or(0, |s| s.to_string_lossy().chars().count());
        let cursor = if text.starts_with('.') || stem == 0 { text.chars().count() } else { stem };
        Self { text: text.to_string(), cursor }
    }

    fn byte(&self, ch: usize) -> usize {
        self.text.char_indices().nth(ch).map_or(self.text.len(), |(i, _)| i)
    }

    pub fn key(&mut self, k: KeyEvent) -> Outcome {
        let ctrl = k.modifiers.contains(KeyModifiers::CONTROL);
        let n = self.text.chars().count();
        match k.code {
            KeyCode::Enter => return Outcome::Submit,
            KeyCode::Esc => return Outcome::Cancel,
            KeyCode::Char('c') if ctrl => return Outcome::Cancel,
            KeyCode::Tab => return Outcome::Browse,
            KeyCode::Char('a') if ctrl => self.cursor = 0,
            KeyCode::Char('e') if ctrl => self.cursor = n,
            KeyCode::Home => self.cursor = 0,
            KeyCode::End => self.cursor = n,
            KeyCode::Left => self.cursor = self.cursor.saturating_sub(1),
            KeyCode::Right => self.cursor = (self.cursor + 1).min(n),
            KeyCode::Char('u') if ctrl => {
                let b = self.byte(self.cursor);
                self.text.replace_range(..b, "");
                self.cursor = 0;
            }
            KeyCode::Char('w') if ctrl => {
                let chars: Vec<char> = self.text.chars().collect();
                let mut i = self.cursor;
                while i > 0 && chars[i - 1] == ' ' {
                    i -= 1;
                }
                while i > 0 && !matches!(chars[i - 1], ' ' | '.' | '-' | '_') {
                    i -= 1;
                }
                let (a, b) = (self.byte(i), self.byte(self.cursor));
                self.text.replace_range(a..b, "");
                self.cursor = i;
            }
            KeyCode::Backspace if self.cursor > 0 => {
                let (a, b) = (self.byte(self.cursor - 1), self.byte(self.cursor));
                self.text.replace_range(a..b, "");
                self.cursor -= 1;
            }
            KeyCode::Delete if self.cursor < n => {
                let (a, b) = (self.byte(self.cursor), self.byte(self.cursor + 1));
                self.text.replace_range(a..b, "");
            }
            KeyCode::Char(c) if !ctrl && c != '/' => {
                let b = self.byte(self.cursor);
                self.text.insert(b, c);
                self.cursor += 1;
            }
            _ => {}
        }
        Outcome::Editing
    }
}

/// O nome serve? `None` se sim; senão, o porquê.
pub fn name_problem(name: &str) -> Option<&'static str> {
    if name.trim().is_empty() {
        Some("o nome está vazio")
    } else if name == "." || name == ".." {
        Some("esse nome não serve")
    } else {
        None
    }
}

fn tilde(p: &Path) -> String {
    let home = std::env::var("HOME").unwrap_or_default();
    let s = p.display().to_string();
    match s.strip_prefix(&home) {
        Some(rest) if !home.is_empty() => format!("~{rest}"),
        _ => s,
    }
}

// ── a tela ────────────────────────────────────────────────────────────────

struct Raw;

impl Raw {
    fn on() -> Result<Self> {
        terminal::enable_raw_mode()?;
        execute!(std::io::stdout(), terminal::EnterAlternateScreen, cursor::Show)?;
        Ok(Raw)
    }
}

impl Drop for Raw {
    fn drop(&mut self) {
        let _ = execute!(std::io::stdout(), terminal::LeaveAlternateScreen);
        let _ = terminal::disable_raw_mode();
    }
}

const DIM: &str = "\x1b[2m";
const BOLD: &str = "\x1b[1m";
const OFF: &str = "\x1b[0m";

/// Pergunta: o campo (se houver) e uma linha de ajuda. Devolve o desfecho e o texto final.
fn prompt(heading: &str, dir: &Path, field: Option<&mut Field>, help: &str, warn: &str) -> Result<Outcome> {
    let _raw = Raw::on()?;
    let mut field = field;
    let mut warn = warn.to_string();
    loop {
        let mut o = std::io::stdout();
        write!(o, "\x1b[2J\x1b[H\r\n\r\n   {BOLD}{heading}{OFF}\r\n\r\n")?;
        write!(o, "   {DIM}pasta{OFF}  {}\r\n", tilde(dir))?;
        let mut cursor_at = None;
        if let Some(f) = field.as_deref() {
            let before: String = f.text.chars().take(f.cursor).collect();
            write!(o, "   {DIM}nome{OFF}   {}\r\n", f.text)?;
            cursor_at = Some(10 + unicode_width::UnicodeWidthStr::width(before.as_str()) as u16);
        }
        write!(o, "\r\n   {DIM}{help}{OFF}\r\n")?;
        if !warn.is_empty() {
            write!(o, "\r\n   {warn}\r\n")?;
        }
        match cursor_at {
            Some(x) => execute!(o, cursor::MoveTo(x, 5), cursor::Show)?,
            None => execute!(o, cursor::Hide)?,
        }
        o.flush()?;
        let Event::Key(k) = event::read()? else { continue };
        if k.kind != KeyEventKind::Press {
            continue;
        }
        let outcome = match field.as_deref_mut() {
            Some(f) => f.key(k),
            None => match k.code {
                KeyCode::Enter => Outcome::Submit,
                KeyCode::Tab => Outcome::Browse,
                KeyCode::Esc => Outcome::Cancel,
                KeyCode::Char('c') if k.modifiers.contains(KeyModifiers::CONTROL) => Outcome::Cancel,
                _ => Outcome::Editing,
            },
        };
        if outcome != Outcome::Editing {
            return Ok(outcome);
        }
        warn.clear();
    }
}

/// O yazi, numa pasta: devolve a pasta onde ele terminou (saindo com `q`), ou `None` com `Q`.
fn browse_dir(start: &Path) -> Result<Option<PathBuf>> {
    let tmp = std::env::temp_dir().join(format!("clios-picker-{}", std::process::id()));
    let _ = std::fs::remove_file(&tmp);
    Command::new("yazi").arg(format!("--cwd-file={}", tmp.display())).arg(start).status().context("abrindo o yazi")?;
    let cwd = std::fs::read_to_string(&tmp).ok().map(|s| PathBuf::from(s.trim())).filter(|p| p.is_dir());
    let _ = std::fs::remove_file(&tmp);
    Ok(cwd)
}

fn start_dir(path: &Path) -> PathBuf {
    let home = PathBuf::from(std::env::var("HOME").unwrap_or_else(|_| "/".into()));
    if path.is_dir() {
        return path.to_path_buf();
    }
    path.parent().filter(|p| p.is_dir()).map_or(home, Path::to_path_buf)
}

fn write_out(out: &Path, paths: &[PathBuf]) -> Result<()> {
    let body: String = paths.iter().map(|p| format!("{}\n", p.display())).collect();
    std::fs::write(out, body).with_context(|| format!("escrevendo {}", out.display()))
}

/// O diálogo em si (roda dentro da janela que o `portal` abriu).
pub fn ui(mode: Mode, path: &Path, out: &Path) -> Result<()> {
    if !is_installed("yazi") {
        bail!("o yazi não está instalado");
    }
    let _ = std::fs::remove_file(out);
    match mode {
        Mode::Open | Mode::Multi => {
            // Enter abre (escolhe); com várias, espaço marca e Enter confirma.
            let st = Command::new("yazi")
                .arg(format!("--chooser-file={}", out.display()))
                .arg(start_dir(path))
                .status()
                .context("abrindo o yazi")?;
            if !st.success() {
                let _ = std::fs::remove_file(out);
            }
            if mode == Mode::Open {
                // o portal quer um só: fica o primeiro
                if let Ok(t) = std::fs::read_to_string(out) {
                    let first = t.lines().next().unwrap_or_default().to_string();
                    std::fs::write(out, if first.is_empty() { String::new() } else { format!("{first}\n") })?;
                }
            }
            Ok(())
        }
        Mode::Dir => {
            let mut dir = start_dir(path);
            loop {
                match browse_dir(&dir)? {
                    None => return Ok(()),
                    Some(d) => dir = d,
                }
                match prompt("usar esta pasta?", &dir, None, "enter usa · tab escolhe outra · esc cancela", "")? {
                    Outcome::Submit => return write_out(out, &[dir]),
                    Outcome::Browse => continue,
                    _ => return Ok(()),
                }
            }
        }
        Mode::Save => {
            let mut dir = start_dir(path);
            let suggested = path.file_name().map_or_else(String::new, |n| n.to_string_lossy().into_owned());
            let mut field = Field::new(&suggested);
            let mut warn = String::new();
            let mut armed: Option<String> = None;
            loop {
                let help = "enter salva · tab escolhe a pasta no yazi (q confirma) · esc cancela";
                match prompt("salvar como", &dir, Some(&mut field), help, &warn)? {
                    Outcome::Cancel => return Ok(()),
                    Outcome::Browse => {
                        if let Some(d) = browse_dir(&dir)? {
                            dir = d;
                        }
                        warn.clear();
                    }
                    Outcome::Submit => {
                        if let Some(why) = name_problem(&field.text) {
                            warn = why.to_string();
                            continue;
                        }
                        let target = dir.join(&field.text);
                        if target.exists() && armed.as_deref() != Some(field.text.as_str()) {
                            warn = format!("{} já existe: enter de novo substitui", sh_quote(&field.text));
                            armed = Some(field.text.clone());
                            continue;
                        }
                        return write_out(out, &[target]);
                    }
                    Outcome::Editing => {}
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn k(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn ctrl(c: char) -> KeyEvent {
        KeyEvent::new(KeyCode::Char(c), KeyModifiers::CONTROL)
    }

    fn s(v: &[&str]) -> Vec<String> {
        v.iter().map(|x| x.to_string()).collect()
    }

    #[test]
    fn the_portal_arguments_pick_the_mode() {
        let p = |a: &[&str]| parse_portal(&s(a)).unwrap().0;
        assert_eq!(p(&["0", "0", "0", "/h", "/o"]), Mode::Open);
        assert_eq!(p(&["1", "0", "0", "/h", "/o"]), Mode::Multi);
        assert_eq!(p(&["0", "1", "0", "/h", "/o"]), Mode::Dir);
        assert_eq!(p(&["0", "0", "1", "/h/a.pdf", "/o", "0"]), Mode::Save);
        assert!(parse_portal(&s(&["0", "0"])).is_err());
    }

    #[test]
    fn the_window_floats_with_its_own_class_and_runs_the_dialog() {
        let v = window_argv("/usr/local/bin/clios", true, Mode::Save, Path::new("/h/a b.pdf"), Path::new("/tmp/o"));
        assert_eq!(v[0], "footclient");
        assert!(v.contains(&"--app-id=clios.float.picker".to_string()));
        assert!(v.contains(&"--title=salvar como".to_string()));
        let tail: Vec<&str> = v[4..].iter().map(String::as_str).collect();
        assert_eq!(
            tail,
            ["/usr/local/bin/clios", "filechooser", "--mode", "save", "--path", "/h/a b.pdf", "--out", "/tmp/o"]
        );
        assert_eq!(window_argv("clios", false, Mode::Open, Path::new("/"), Path::new("/o"))[0], "foot");
    }

    #[test]
    fn the_cursor_starts_before_the_extension() {
        assert_eq!(Field::new("relatório.pdf").cursor, 9);
        assert_eq!(Field::new(".bashrc").cursor, 7);
        assert_eq!(Field::new("").cursor, 0);
    }

    #[test]
    fn editing_works_like_the_shell() {
        let mut f = Field::new("foto.png");
        for c in "-final".chars() {
            f.key(k(KeyCode::Char(c)));
        }
        assert_eq!(f.text, "foto-final.png");
        f.key(ctrl('w'));
        assert_eq!(f.text, "foto-.png");
        f.key(k(KeyCode::Backspace));
        assert_eq!(f.text, "foto.png");
        f.key(ctrl('e'));
        f.key(k(KeyCode::Char('/')));
        assert_eq!(f.text, "foto.png", "barra não entra no nome");
        f.key(ctrl('a'));
        f.key(k(KeyCode::Delete));
        assert_eq!(f.text, "oto.png");
        f.key(k(KeyCode::End));
        f.key(ctrl('u'));
        assert_eq!((f.text.as_str(), f.cursor), ("", 0));
        assert_eq!(f.key(k(KeyCode::Enter)), Outcome::Submit);
        assert_eq!(f.key(k(KeyCode::Tab)), Outcome::Browse);
        assert_eq!(f.key(k(KeyCode::Esc)), Outcome::Cancel);
        assert_eq!(f.key(ctrl('c')), Outcome::Cancel);
    }

    #[test]
    fn accents_are_one_character_each() {
        let mut f = Field::new("ação");
        f.key(k(KeyCode::Backspace));
        assert_eq!(f.text, "açã");
        f.key(k(KeyCode::Left));
        f.key(k(KeyCode::Char('ç')));
        assert_eq!(f.text, "aççã");
    }

    #[test]
    fn names_are_checked() {
        assert!(name_problem("  ").is_some());
        assert!(name_problem("..").is_some());
        assert!(name_problem("nota.md").is_none());
    }

    #[test]
    fn the_start_folder_falls_back_sensibly() {
        assert_eq!(start_dir(Path::new("/")), PathBuf::from("/"));
        assert_eq!(start_dir(Path::new("/tmp/um-arquivo-que-nao-existe.txt")), PathBuf::from("/tmp"));
    }
}
