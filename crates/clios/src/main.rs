//! clios: tema, hub e utilitários do desktop feito para o terminal.

mod art;
mod catalog;
mod cmd;
mod ctx;
mod hub;
mod sys;
mod sysinfo;
mod toys;
mod ui;
mod welcome;

use std::path::PathBuf;
use std::process::ExitCode;

use anyhow::Result;
use clap::{Parser, Subcommand};
use clios_core::{Mode, MotionLevel};

use ctx::Ctx;

#[derive(Parser)]
#[command(name = "clios", version, about = "Tema, hub e utilitários do CLIOS", arg_required_else_help = true)]
struct Cli {
    /// Diretório do CLIOS (o checkout). Padrão: $CLIOS_ROOT, o checkout deste binário, ~/.local/share/clios.
    #[arg(long, global = true, value_name = "DIR")]
    root: Option<PathBuf>,

    /// Trata DIR como a pasta pessoal (para montar a ISO e testar sem tocar no seu ~).
    #[arg(long, global = true, value_name = "DIR")]
    home: Option<PathBuf>,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Modo, acento e movimento: escolhe e aplica em todos os apps.
    Theme {
        #[command(subcommand)]
        command: ThemeCommand,
    },
    /// Quanto movimento o desktop faz (sem argumento, alterna full → reduced → off).
    Motion {
        #[arg(value_name = "full|reduced|off")]
        level: Option<MotionLevel>,
    },
    /// Instala os dotfiles e renderiza o tema.
    Sync {
        /// Copia os arquivos em vez de criar links (para a ISO).
        #[arg(long)]
        copy: bool,
        /// Mostra o que faria, sem mudar nada.
        #[arg(long)]
        dry_run: bool,
        /// Só liga os dotfiles, sem renderizar o tema.
        #[arg(long)]
        no_theme: bool,
    },
    /// Abre o lançador e paleta de comandos (use dentro de uma janela de terminal).
    Hub {
        /// Imprime os resultados da consulta e sai.
        #[arg(long, value_name = "CONSULTA")]
        dump: Option<String>,
        #[arg(long, value_name = "LARGURAxALTURA", hide = true)]
        snapshot: Option<String>,
        /// Consulta inicial (ex.: ">" abre direto nas ações).
        #[arg(long, default_value = "")]
        query: String,
        #[arg(long, default_value_t = 60_000, hide = true)]
        at: u64,
        /// Para capturas da documentação: só o que vem instalado de fábrica.
        #[arg(long, hide = true)]
        demo: bool,
    },
    /// Abre uma entrada do catálogo do hub numa janela de terminal (é o que os atalhos chamam).
    Open {
        /// `hub` ou o id de uma entrada [[tui]] do catálogo (files, git, audio...).
        id: String,
        /// Para `hub`: a consulta inicial.
        args: Vec<String>,
    },
    /// Papel de parede: estilos que seguem o tema e as suas imagens (sem argumento, abre o seletor).
    Wallpaper {
        #[command(subcommand)]
        command: Option<WallpaperCommand>,
        /// Desenha um quadro do seletor em ANSI e sai: LARGURAxALTURA (docs e capturas).
        #[arg(long, value_name = "LARGURAxALTURA", hide = true)]
        snapshot: Option<String>,
        /// Com --snapshot: o id do estilo selecionado.
        #[arg(long, hide = true, requires = "snapshot")]
        select: Option<String>,
    },
    /// O catálogo de apps curados: lista, mostra e instala.
    Apps {
        #[command(subcommand)]
        command: AppsCommand,
    },
    /// O guia de boas-vindas e a central do sistema.
    Welcome {
        /// Página inicial: início, atalhos, apps, sistema, dicas ou sobre.
        #[arg(long, short)]
        page: Option<String>,
        /// Para o autostart: abre uma janela só se o guia nunca foi visto.
        #[arg(long)]
        first_run: bool,
        #[arg(long, value_name = "LARGURAxALTURA", hide = true)]
        snapshot: Option<String>,
        /// Com --snapshot: o item selecionado da página (grupo, categoria, linha ou dica).
        #[arg(long, hide = true)]
        select: Option<usize>,
        /// Com --snapshot: mostra o que vem instalado de fábrica e os primeiros passos adiantados.
        #[arg(long, hide = true, requires = "snapshot")]
        demo: bool,
    },
    /// O resumo do sistema, com a marca ao lado.
    Fetch,
    /// A apresentação do terminal: o resumo ao ligar, duas linhas em workspace vazia (o fish chama ao abrir).
    Greet {
        /// Mostra agora, sem decidir nem gravar: fetch ou line.
        #[arg(long, value_enum)]
        show: Option<cmd::greet::Greeting>,
        /// Esquece o que já foi mostrado nesta sessão.
        #[arg(long, conflicts_with = "show")]
        reset: bool,
        /// Quando se apresentar: all (ao ligar e em workspace vazia), boot (só ao ligar) ou off.
        #[arg(long, conflicts_with_all = ["show", "reset"])]
        mode: Option<clios_core::GreetMode>,
    },
    /// Lê as notícias do Arch, atualiza o sistema e avisa dos .pacnew.
    Update {
        /// Só mostra as notícias, sem atualizar.
        #[arg(long)]
        news: bool,
        /// Não pergunta antes de atualizar (as notícias continuam aparecendo).
        #[arg(long, short)]
        yes: bool,
    },
    /// Atualiza o próprio CLIOS: puxa o repositório, recompila, reinstala e sincroniza.
    SelfUpdate {
        /// Só diz se há novidades, sem atualizar.
        #[arg(long)]
        check: bool,
    },
    /// Modo café: a tela não apaga e o sistema não suspende.
    Caffeine {
        #[arg(value_enum, default_value = "toggle")]
        switch: cmd::toggles::Switch,
    },
    /// Os atalhos do desktop, no terminal (com um filtro, mostra só o que combina: `clios keys captura`).
    Keys {
        /// Parte do nome do grupo, da tecla ou da descrição.
        query: Vec<String>,
    },
    /// Um bloco de foco: liga o não perturbe, a barra mostra quanto falta e, no fim, avisa (sem argumento: 25 minutos, ou para).
    Focus {
        /// Minutos (1 a 240) ou `stop`.
        #[arg(value_name = "MINUTOS|stop")]
        what: Option<String>,
        /// Mostra só o que falta, em texto (para scripts).
        #[arg(long, conflicts_with_all = ["what", "wait"])]
        status: bool,
        /// O processo que espera o fim; quem chama é o próprio `clios focus`.
        #[arg(long, hide = true)]
        wait: bool,
    },
    /// Modo noturno: tela mais quente (hyprsunset).
    Night {
        #[arg(value_enum, default_value = "toggle")]
        switch: cmd::toggles::Switch,
        /// Temperatura em kelvin (1000 a 10000).
        #[arg(long, short)]
        temp: Option<u32>,
    },
    /// Não perturbe: silencia as notificações (as urgentes passam).
    Dnd {
        #[arg(value_enum, default_value = "toggle")]
        switch: cmd::toggles::Switch,
    },
    /// Grava a tela (sem argumento: começa por uma região, ou para se já está gravando).
    Rec {
        #[arg(value_enum, default_value = "toggle")]
        what: cmd::toggles::RecWhat,
        /// Grava também o áudio.
        #[arg(long)]
        audio: bool,
    },
    /// Conta-gotas: escolhe uma cor da tela e copia o hex.
    Pick,
    /// Seleciona uma região da tela e copia o texto que está nela (OCR).
    Ocr,
    /// Perfil de energia (sem argumento, passa para o próximo).
    Power {
        #[arg(value_name = "power-saver|balanced|performance")]
        profile: Option<String>,
    },
    /// Proteção de tela: a marca ou um brinquedo, o que você escolheu (sem argumento, abre agora).
    Saver {
        #[command(subcommand)]
        command: Option<SaverCommand>,
        /// Roda aqui, no terminal atual (é o que a janela em tela cheia executa).
        #[arg(long)]
        run: bool,
        /// Força uma cena nesta vez: marca ou o id de um brinquedo (veja `clios saver list`).
        #[arg(long, value_name = "CENA")]
        scene: Option<String>,
    },
    /// Roda um brinquedo aqui no terminal, na cor do seu acento (sem argumento, sorteia um dos instalados).
    Play {
        /// O id do brinquedo (veja `clios play --list`).
        id: Option<String>,
        /// Lista os brinquedos e o que falta instalar.
        #[arg(long, short)]
        list: bool,
    },
    /// O histórico de notificações, inclusive as que o "não perturbe" silenciou.
    Notifs {
        #[command(subcommand)]
        command: Option<NotifsCommand>,
        /// Quantas mostrar (as mais novas).
        #[arg(long, short, default_value_t = 30)]
        count: usize,
    },
    /// Estilo do prompt do shell: minimal, dev ou zen (sem argumento, lista).
    Prompt { style: Option<clios_core::PromptStyle> },
    /// Imprime o autocompletar para o seu shell (fish, bash, zsh): `clios completions fish > ~/.config/fish/completions/clios.fish`.
    Completions {
        #[arg(value_enum)]
        shell: clap_complete::Shell,
    },
    /// Confere o que falta para o desktop funcionar.
    Doctor,
    /// Estado do sistema em JSON, para a barra.
    Status {
        #[command(subcommand)]
        what: StatusWhat,
    },
    /// Captura de tela: salva, copia e avisa.
    Shot {
        #[arg(value_enum, default_value = "region")]
        target: ShotTarget,
    },
}

#[derive(Subcommand)]
enum ThemeCommand {
    /// Renderiza tudo de novo com a escolha salva.
    Apply {
        /// Não recolore os terminais que já estão abertos.
        #[arg(long)]
        no_live: bool,
        /// Não avisa Hyprland, helix e btop.
        #[arg(long)]
        no_hooks: bool,
        #[arg(long)]
        dry_run: bool,
    },
    /// Escolhe modo, acento e/ou movimento.
    Set {
        #[arg(long, value_name = "dark|light")]
        mode: Option<Mode>,
        /// Nome de um acento (veja `clios theme list`) ou um hex como #7CFF00.
        #[arg(long, value_name = "NOME|#HEX")]
        accent: Option<String>,
        #[arg(long, value_name = "full|reduced|off")]
        motion: Option<MotionLevel>,
    },
    /// Alterna entre escuro e claro.
    Toggle,
    /// Passa para o próximo acento.
    Cycle,
    /// Lista os acentos.
    List,
    /// Mostra a paleta atual com a razão de contraste de cada cor.
    Show,
    /// Parâmetros `vt.default_*` para o console do kernel usar a mesma paleta.
    Cmdline,
}

#[derive(Subcommand)]
enum WallpaperCommand {
    /// Lista os estilos e as suas imagens, marcando o que está em uso.
    List,
    /// Aplica um estilo (veja `list`), uma imagem sua pelo nome, ou o caminho de uma imagem nova.
    Set { what: String },
    /// Passa para o próximo.
    Next,
    /// Volta para o anterior.
    Prev,
    /// Sorteia um diferente do atual.
    Random,
    /// Copia uma imagem para o CLIOS (em ~/.local/share/clios/wallpapers).
    Add {
        file: PathBuf,
        /// Já aplica.
        #[arg(long)]
        set: bool,
    },
    /// Tira uma imagem sua do CLIOS.
    Remove { name: String },
    /// Garante que a imagem do tema atual existe e imprime o caminho dela.
    Path,
}

#[derive(Subcommand)]
enum AppsCommand {
    /// Lista os apps por categoria, marcando os instalados.
    List {
        /// Só uma categoria (arquivos, codigo, sistema, rede, midia, ler, falar, produtividade, pacotes, diversao).
        #[arg(long, short)]
        category: Option<String>,
        /// Só os que faltam.
        #[arg(long, conflicts_with = "installed")]
        missing: bool,
        /// Só os instalados.
        #[arg(long)]
        installed: bool,
    },
    /// Descrição, dica e comando de um app.
    Info { id: String },
    /// Instala por id; com --extras, todos os recomendados que faltam.
    Install {
        ids: Vec<String>,
        /// Todos os apps de nível `extra` que ainda não estão instalados.
        #[arg(long)]
        extras: bool,
        /// Mostra o comando e não executa.
        #[arg(long)]
        dry_run: bool,
    },
}

#[derive(Subcommand)]
enum SaverCommand {
    /// Lista as cenas e mostra a escolhida.
    List,
    /// Escolhe: auto (rodízio), marca, off ou o id de um brinquedo.
    Set { scene: String },
    /// Fecha a proteção de tela aberta.
    Stop,
}

#[derive(Subcommand)]
enum NotifsCommand {
    /// Apaga o histórico.
    Clear,
    /// Registra uma notificação (a shell chama isto sozinha a cada uma que chega).
    #[command(hide = true)]
    Add {
        #[arg(long = "app", default_value = "")]
        app: String,
        #[arg(long = "summary", default_value = "")]
        summary: String,
        #[arg(long = "body", default_value = "")]
        body: String,
        #[arg(long = "urgency", default_value = "normal")]
        urgency: String,
        #[arg(long)]
        silenced: bool,
    },
}

#[derive(Subcommand)]
enum StatusWhat {
    /// Conexão de rede: {"kind":"wifi|eth|none","label":"SSID"}.
    Net,
    /// Rede e os liga-desliga (café, noturno, não perturbe, gravação), para a barra.
    All,
}

#[derive(Clone, Copy, clap::ValueEnum)]
enum ShotTarget {
    Region,
    Screen,
    Window,
}

fn run() -> Result<bool> {
    let cli = Cli::parse();
    let mut ctx = Ctx::load(cli.root.as_deref(), cli.home.as_deref())?;

    match cli.command {
        Command::Theme { command } => match command {
            ThemeCommand::Apply { no_live, no_hooks, dry_run } => {
                let s =
                    cmd::theme::apply(&ctx, cmd::theme::ApplyOptions { live: !no_live, hooks: !no_hooks, dry_run })?;
                cmd::theme::print_summary(&ctx, &s)?;
            }
            ThemeCommand::Set { mode, accent, motion } => cmd::theme::set(&mut ctx, mode, accent, motion)?,
            ThemeCommand::Toggle => cmd::theme::toggle(&mut ctx)?,
            ThemeCommand::Cycle => cmd::theme::cycle_accent(&mut ctx)?,
            ThemeCommand::List => cmd::theme::list(&ctx)?,
            ThemeCommand::Show => cmd::theme::show(&ctx)?,
            ThemeCommand::Cmdline => cmd::theme::cmdline(&ctx)?,
        },
        Command::Motion { level } => cmd::motion::run(&mut ctx, level)?,
        Command::Sync { copy, dry_run, no_theme } => {
            cmd::sync::run(&ctx, cmd::sync::Options { copy, dry_run, no_theme })?
        }
        Command::Hub { dump, snapshot, query, at, demo } => {
            hub::run(&ctx, hub::Options { dump, snapshot, query, at_ms: at, demo })?
        }
        Command::Open { id, args } => hub::open(&ctx, &id, &args)?,
        Command::Status { what: StatusWhat::Net } => cmd::status::run_net()?,
        Command::Status { what: StatusWhat::All } => println!("{}", serde_json::to_string(&cmd::toggles::all(&ctx))?),
        Command::Wallpaper { command, snapshot: Some(size), select } => {
            anyhow::ensure!(command.is_none(), "--snapshot não combina com subcomando");
            cmd::wallpaper_tui::snapshot(&ctx, &size, select.as_deref())?
        }
        Command::Wallpaper { command, .. } => match command {
            None => cmd::wallpaper::pick(&mut ctx)?,
            Some(WallpaperCommand::List) => cmd::wallpaper::list(&ctx)?,
            Some(WallpaperCommand::Set { what }) => cmd::wallpaper::set(&mut ctx, &what)?,
            Some(WallpaperCommand::Next) => cmd::wallpaper::step_cmd(&mut ctx, 1)?,
            Some(WallpaperCommand::Prev) => cmd::wallpaper::step_cmd(&mut ctx, -1)?,
            Some(WallpaperCommand::Random) => cmd::wallpaper::random(&mut ctx)?,
            Some(WallpaperCommand::Add { file, set }) => cmd::wallpaper::add(&mut ctx, &file, set)?,
            Some(WallpaperCommand::Remove { name }) => cmd::wallpaper::remove(&mut ctx, &name)?,
            Some(WallpaperCommand::Path) => cmd::wallpaper::path(&ctx)?,
        },
        Command::Apps { command } => match command {
            AppsCommand::List { category, missing, installed } => {
                let filter = if missing {
                    cmd::apps::Filter::Missing
                } else if installed {
                    cmd::apps::Filter::Installed
                } else {
                    cmd::apps::Filter::All
                };
                cmd::apps::list(&ctx, filter, category.as_deref())?
            }
            AppsCommand::Info { id } => cmd::apps::info(&ctx, &id)?,
            AppsCommand::Install { ids, extras, dry_run } => {
                if ids.is_empty() && !extras {
                    anyhow::bail!("diga o que instalar: clios apps install <id>... ou --extras");
                }
                return cmd::apps::install(&ctx, &ids, extras, dry_run);
            }
        },
        Command::Welcome { page, first_run, snapshot, select, demo } => {
            welcome::run(ctx, welcome::Options { page, first_run, snapshot, select, demo })?
        }
        Command::Fetch => cmd::fetch::run(&ctx)?,
        Command::Greet { show, reset, mode } => {
            if let Some(m) = mode {
                ctx.state.greet = m;
                ctx.save_state()?;
                println!("saudação do terminal: {}", cmd::greet::describe(m));
            } else {
                cmd::greet::run(&ctx, show, reset)?
            }
        }
        Command::Update { news, yes } => return cmd::update::run(&ctx, news, yes),
        Command::SelfUpdate { check } => return cmd::selfupdate::run(&ctx, check),
        Command::Caffeine { switch } => {
            println!("café {}", cmd::toggles::state_word(cmd::toggles::caffeine(&ctx, switch)?))
        }
        Command::Keys { query } => cmd::keys::run(&query.join(" "))?,
        Command::Focus { what, status, wait } => {
            if wait {
                cmd::focus::wait(&ctx)?
            } else if status {
                println!("{}", cmd::focus::status_text(&ctx))
            } else {
                cmd::focus::run(&ctx, what.as_deref())?
            }
        }
        Command::Night { switch, temp } => {
            println!("noturno {}", cmd::toggles::state_word(cmd::toggles::night(&ctx, switch, temp)?))
        }
        Command::Dnd { switch } => {
            println!("não perturbe {}", cmd::toggles::state_word(cmd::toggles::dnd(&ctx, switch)?))
        }
        Command::Rec { what, audio } => cmd::toggles::rec(&ctx, what, audio)?,
        Command::Ocr => cmd::ocr::run(&ctx)?,
        Command::Pick => cmd::toggles::pick()?,
        Command::Power { profile } => cmd::toggles::power(profile.as_deref())?,
        Command::Saver { command: Some(c), .. } => match c {
            SaverCommand::List => cmd::saver::list(&ctx)?,
            SaverCommand::Set { scene } => cmd::saver::set(&mut ctx, &scene)?,
            SaverCommand::Stop => cmd::saver::stop(),
        },
        Command::Saver { command: None, run, scene } => {
            if run {
                cmd::saver::run(&ctx, scene.as_deref())?
            } else {
                cmd::saver::launch(&ctx, scene.as_deref())?
            }
        }
        Command::Play { id, list } => return cmd::play::run(&ctx, id.as_deref(), list),
        Command::Notifs { command, count } => match command {
            None => cmd::notifs::list(&ctx, count)?,
            Some(NotifsCommand::Clear) => cmd::notifs::clear(&ctx)?,
            Some(NotifsCommand::Add { app, summary, body, urgency, silenced }) => {
                let t = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs());
                cmd::notifs::add(&ctx, cmd::notifs::Entry { t, app, summary, body, urgency, silenced })?
            }
        },
        Command::Prompt { style } => cmd::prompt::run(&mut ctx, style)?,
        Command::Completions { shell } => {
            use clap::CommandFactory;
            use std::io::Write;
            let mut buf = Vec::new();
            clap_complete::generate(shell, &mut Cli::command(), "clios", &mut buf);
            if shell == clap_complete::Shell::Fish {
                buf.extend_from_slice(cmd::completions::fish_ids(&catalog::builtin()).as_bytes());
            }
            // `clios completions fish | head` fecha o cano antes do fim: não é erro.
            let _ = std::io::stdout().write_all(&buf);
        }
        Command::Doctor => return cmd::doctor::run(&ctx),
        Command::Shot { target } => {
            let t = match target {
                ShotTarget::Region => cmd::shot::Target::Region,
                ShotTarget::Screen => cmd::shot::Target::Screen,
                ShotTarget::Window => cmd::shot::Target::Window,
            };
            cmd::shot::run(&ctx, t)?;
        }
    }
    Ok(true)
}

fn main() -> ExitCode {
    match run() {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::FAILURE,
        Err(e) => {
            eprintln!("clios: {e:#}");
            ExitCode::FAILURE
        }
    }
}
