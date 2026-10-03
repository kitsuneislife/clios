//! O que o hub lista e o que cada linha faz.

use clios_core::{Rgb, Theme};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Kind {
    App,
    Tui,
    Action,
    Window,
    Key,
    Clip,
    /// App do catálogo que ainda não está instalado.
    Install,
    /// O resultado de uma conta (`=`), calculado pelo fend.
    Calc,
}

impl Kind {
    pub fn label(self) -> &'static str {
        match self {
            Kind::App => "app",
            Kind::Tui => "tui",
            Kind::Action => "ação",
            Kind::Window => "janela",
            Kind::Key => "atalho",
            Kind::Clip => "clip",
            Kind::Install => "instalar",
            Kind::Calc => "conta",
        }
    }
}

/// Qual conjunto a busca olha. Escolhido pelo primeiro caractere da consulta.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Scope {
    #[default]
    All,
    Windows,
    Keys,
    Actions,
    Clipboard,
    Install,
    Calc,
}

impl Scope {
    pub const CYCLE: [Scope; 7] =
        [Scope::All, Scope::Windows, Scope::Keys, Scope::Actions, Scope::Clipboard, Scope::Install, Scope::Calc];

    pub fn prefix(self) -> &'static str {
        match self {
            Scope::All => "",
            Scope::Windows => "@",
            Scope::Keys => "?",
            Scope::Actions => ">",
            Scope::Clipboard => "\"",
            Scope::Install => "+",
            Scope::Calc => "=",
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Scope::All => "tudo",
            Scope::Windows => "janelas",
            Scope::Keys => "atalhos",
            Scope::Actions => "ações",
            Scope::Clipboard => "clipboard",
            Scope::Install => "instalar",
            Scope::Calc => "conta",
        }
    }

    /// Separa o prefixo de escopo do resto da consulta: `"@fire"` vira `(Windows, "fire")`.
    pub fn split(query: &str) -> (Scope, &str) {
        for s in Self::CYCLE {
            if !s.prefix().is_empty() {
                if let Some(rest) = query.strip_prefix(s.prefix()) {
                    return (s, rest.trim_start());
                }
            }
        }
        (Scope::All, query)
    }

    pub fn next(self, back: bool) -> Scope {
        let i = Self::CYCLE.iter().position(|s| *s == self).unwrap_or(0);
        let n = Self::CYCLE.len();
        Self::CYCLE[if back { (i + n - 1) % n } else { (i + 1) % n }]
    }

    pub fn includes(self, kind: Kind) -> bool {
        match self {
            Scope::All => matches!(kind, Kind::App | Kind::Tui | Kind::Action),
            Scope::Windows => kind == Kind::Window,
            Scope::Keys => kind == Kind::Key,
            Scope::Actions => kind == Kind::Action,
            Scope::Clipboard => kind == Kind::Clip,
            Scope::Install => kind == Kind::Install,
            Scope::Calc => kind == Kind::Calc,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    /// Informativo (atalhos): Enter não faz nada.
    None,
    /// Programa gráfico, via `sh -c`.
    Gui(String),
    /// Programa de terminal numa janela nova do foot.
    Tui {
        id: String,
        argv: Vec<String>,
        float: bool,
        hold: bool,
    },
    /// Roda o próprio `clios` com estes argumentos.
    Clios(Vec<String>),
    Shell(String),
    /// `hyprctl dispatch <expressão Lua>`.
    Hypr(String),
    /// Entrada do cliphist: copia de volta para a área de transferência.
    Clip(String),
    /// Muda o escopo da busca sem fechar o hub.
    Scope(Scope),
    /// Copia o texto para a área de transferência (e avisa).
    Copy(String),
}

#[derive(Debug, Clone)]
pub struct Item {
    pub kind: Kind,
    /// Chave estável para o histórico de uso (`app:firefox`).
    pub id: String,
    pub title: String,
    /// Texto à direita, antes do tipo (atalho, workspace).
    pub hint: String,
    pub keywords: String,
    pub action: Action,
    /// Pede um segundo Enter (desligar, sair).
    pub confirm: bool,
    /// Bolinha de cor antes do título (seletor de acento).
    pub swatch: Option<Rgb>,
}

impl Item {
    pub fn new(kind: Kind, id: impl Into<String>, title: impl Into<String>, action: Action) -> Self {
        Self {
            kind,
            id: id.into(),
            title: title.into(),
            hint: String::new(),
            keywords: String::new(),
            action,
            confirm: false,
            swatch: None,
        }
    }

    pub fn keywords(mut self, k: impl Into<String>) -> Self {
        self.keywords = k.into();
        self
    }

    pub fn hint(mut self, h: impl Into<String>) -> Self {
        self.hint = h.into();
        self
    }

    pub fn confirm(mut self) -> Self {
        self.confirm = true;
        self
    }

    pub fn swatch(mut self, c: Rgb) -> Self {
        self.swatch = Some(c);
        self
    }
}

/// Ações embutidas: tema, sessão, capturas.
pub fn builtin_actions(theme: &Theme) -> Vec<Item> {
    let a = |id: &str, title: &str, kw: &str, args: &[&str]| {
        Item::new(
            Kind::Action,
            format!("action:{id}"),
            title,
            Action::Clios(args.iter().map(|s| s.to_string()).collect()),
        )
        .keywords(kw)
    };
    let sh = |id: &str, title: &str, kw: &str, cmd: &str| {
        Item::new(Kind::Action, format!("action:{id}"), title, Action::Shell(cmd.to_string())).keywords(kw)
    };

    let mut v = vec![
        a("toggle", "tema: alternar claro e escuro", "theme dark light escuro claro modo", &["theme", "toggle"]),
        a("motion-cycle", "movimento: alternar nível", "motion animation animacao reduzido", &["motion"]),
        a("motion-full", "movimento: completo", "motion animation full", &["motion", "full"]),
        a("motion-reduced", "movimento: reduzido", "motion reduced fade", &["motion", "reduced"]),
        a("motion-off", "movimento: desligado", "motion off none", &["motion", "off"]),
        a("wallpaper-next", "papel de parede: próximo", "wallpaper fundo background", &["wallpaper", "next"]),
        a(
            "wallpaper-random",
            "papel de parede: sortear",
            "wallpaper fundo background random aleatorio",
            &["wallpaper", "random"],
        ),
        a("focus", "foco: 25 minutos em silêncio", "focus foco pomodoro concentrar", &["focus"]),
        a("focus-50", "foco: 50 minutos em silêncio", "focus foco pomodoro concentrar", &["focus", "50"]),
        a("focus-stop", "foco: parar", "focus foco parar", &["focus", "stop"]),
        a("caffeine", "modo café: ligar ou desligar", "caffeine cafe tela acordado inhibit", &["caffeine"]),
        a("night", "modo noturno: ligar ou desligar", "night noturno quente hyprsunset redshift", &["night"]),
        a(
            "night-auto",
            "modo noturno: todo dia das 20:30 às 06:45",
            "night noturno agendar horario automatico",
            &["night", "auto", "20:30-06:45"],
        ),
        a("night-auto-off", "modo noturno: só à mão", "night noturno agendado desligar", &["night", "auto", "off"]),
        a(
            "session-restore",
            "sessão: reabrir as janelas salvas",
            "session sessao restaurar janelas",
            &["session", "restore"],
        ),
        a("session-on", "sessão: reabrir no login", "session sessao login restaurar", &["session", "on"]),
        a("session-off", "sessão: começar com a mesa limpa", "session sessao login limpa", &["session", "off"]),
        a(
            "snap-new",
            "fotografia do sistema: tirar agora",
            "snap snapshot snapper fotografia backup",
            &["snap", "new", "hub"],
        ),
        a("dnd", "não perturbe: ligar ou desligar", "dnd silencio notificacoes mute", &["dnd"]),
        a("rec", "gravar a tela: região", "rec record gravar video wf-recorder", &["rec", "region"]),
        a("rec-screen", "gravar a tela: inteira", "rec record gravar video", &["rec", "screen"]),
        a("rec-stop", "gravar a tela: parar", "rec stop parar gravacao", &["rec", "stop"]),
        a("ocr", "texto da tela: copiar uma região (OCR)", "ocr texto imagem tesseract copiar ler", &["ocr"]),
        a("pick", "conta-gotas: copiar uma cor da tela", "color picker cor hex hyprpicker", &["pick"]),
        a("power-cycle", "energia: próximo perfil", "power profile bateria desempenho economia", &["power"]),
        a("saver", "proteção de tela: abrir agora", "screensaver saver descanso", &["saver"]),
        a("saver-auto", "proteção de tela: rodízio", "screensaver saver auto rodizio", &["saver", "set", "auto"]),
        a("saver-mark", "proteção de tela: só a marca", "screensaver saver marca logo", &["saver", "set", "marca"]),
        a("saver-off", "proteção de tela: desligar", "screensaver saver off desligar", &["saver", "set", "off"]),
        a("prompt-minimal", "prompt: minimal", "prompt starship minimal uma linha", &["prompt", "minimal"]),
        a("prompt-dev", "prompt: dev", "prompt starship dev duas linhas linguagens", &["prompt", "dev"]),
        a("prompt-zen", "prompt: zen", "prompt starship zen so seta", &["prompt", "zen"]),
        a(
            "greet-all",
            "saudação do terminal: ao ligar e em workspace vazia",
            "greet fastfetch saudacao",
            &["greet", "--mode", "all"],
        ),
        a("greet-boot", "saudação do terminal: só ao ligar", "greet fastfetch saudacao", &["greet", "--mode", "boot"]),
        a("greet-off", "saudação do terminal: nunca", "greet fastfetch saudacao desligar", &["greet", "--mode", "off"]),
        a("shot-region", "captura: região", "screenshot print screen recorte", &["shot", "region"]),
        a("shot-screen", "captura: tela inteira", "screenshot print screen", &["shot", "screen"]),
        a("shot-window", "captura: janela", "screenshot print window", &["shot", "window"]),
        sh("lock", "bloquear", "lock tela hyprlock", "pidof hyprlock || hyprlock"),
        sh("suspend", "suspender", "sleep suspend", "systemctl suspend"),
        sh(
            "reload-shell",
            "recarregar a shell",
            "quickshell restart barra",
            "pkill -x quickshell; sleep 0.3; setsid -f quickshell",
        ),
        Item::new(Kind::Action, "action:keys", "ver atalhos", Action::Scope(Scope::Keys))
            .keywords("keys binds help ajuda"),
        Item::new(
            Kind::Action,
            "action:doctor",
            "diagnóstico do sistema",
            Action::Tui { id: "doctor".into(), argv: vec!["clios".into(), "doctor".into()], float: true, hold: true },
        )
        .keywords("doctor check health"),
        Item::new(Kind::Action, "action:logout", "sair da sessão", Action::Hypr("hl.dsp.exit()".into()))
            .keywords("logout exit sair")
            .confirm(),
        sh("reboot", "reiniciar", "reboot restart", "systemctl reboot").confirm(),
        sh("poweroff", "desligar", "shutdown poweroff", "systemctl poweroff").confirm(),
    ];

    for s in clios_core::wallpaper::Style::ALL {
        v.push(a(
            &format!("wallpaper-{}", s.id()),
            &format!("papel de parede: {}", s.name()),
            "wallpaper fundo background tela",
            &["wallpaper", "set", s.id()],
        ));
    }
    for s in &theme.accents {
        let current = s.name == theme.accent_name;
        v.push(
            a(
                &format!("accent-{}", s.name),
                &format!("acento: {}{}", s.name, if current { "  (atual)" } else { "" }),
                "accent color cor tema",
                &["theme", "set", "--accent", &s.name],
            )
            .swatch(s.color),
        );
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;
    use clios_core::{Mode, MotionLevel, Tokens};

    #[test]
    fn scope_prefixes() {
        assert_eq!(Scope::split("@fire"), (Scope::Windows, "fire"));
        assert_eq!(Scope::split("? super"), (Scope::Keys, "super"));
        assert_eq!(Scope::split(">tema"), (Scope::Actions, "tema"));
        assert_eq!(Scope::split("\"senha"), (Scope::Clipboard, "senha"));
        assert_eq!(Scope::split("+cava"), (Scope::Install, "cava"));
        assert_eq!(Scope::split("firefox"), (Scope::All, "firefox"));
        assert_eq!(Scope::split(""), (Scope::All, ""));
    }

    #[test]
    fn scope_cycle_wraps_both_ways() {
        assert_eq!(Scope::All.next(false), Scope::Windows);
        assert_eq!(Scope::Calc.next(false), Scope::All);
        assert_eq!(Scope::Clipboard.next(false), Scope::Install);
        assert_eq!(Scope::All.next(true), Scope::Calc);
    }

    #[test]
    fn default_scope_hides_windows_keys_and_clips() {
        assert!(Scope::All.includes(Kind::App) && Scope::All.includes(Kind::Tui) && Scope::All.includes(Kind::Action));
        assert!(
            !Scope::All.includes(Kind::Window) && !Scope::All.includes(Kind::Key) && !Scope::All.includes(Kind::Clip)
        );
    }

    #[test]
    fn dangerous_actions_ask_for_confirmation() {
        let t = Theme::resolve(&Tokens::builtin(), Mode::Dark, "ember", MotionLevel::Full).unwrap();
        let acts = builtin_actions(&t);
        for id in ["action:logout", "action:reboot", "action:poweroff"] {
            assert!(acts.iter().find(|i| i.id == id).unwrap().confirm, "{id}");
        }
        assert!(!acts.iter().find(|i| i.id == "action:lock").unwrap().confirm);
    }

    #[test]
    fn one_accent_item_per_accent_and_current_is_marked() {
        let t = Theme::resolve(&Tokens::builtin(), Mode::Dark, "azure", MotionLevel::Full).unwrap();
        let acts = builtin_actions(&t);
        let accents: Vec<_> = acts.iter().filter(|i| i.id.starts_with("action:accent-")).collect();
        assert_eq!(accents.len(), 7);
        assert!(accents.iter().all(|i| i.swatch.is_some()));
        let azure = accents.iter().find(|i| i.id == "action:accent-azure").unwrap();
        assert!(azure.title.contains("(atual)"));
        assert_eq!(accents.iter().filter(|i| i.title.contains("(atual)")).count(), 1);
    }

    #[test]
    fn item_ids_are_unique() {
        let t = Theme::resolve(&Tokens::builtin(), Mode::Dark, "ember", MotionLevel::Full).unwrap();
        let mut ids: Vec<_> = builtin_actions(&t).into_iter().map(|i| i.id).collect();
        let n = ids.len();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), n, "ids duplicados quebram o histórico de uso");
    }
}
