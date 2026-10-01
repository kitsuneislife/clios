//! clios: tema, hub e utilitários do desktop feito para o terminal.

mod catalog;
mod cmd;
mod ctx;
mod hub;
mod sys;
mod ui;

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
enum StatusWhat {
    /// Conexão de rede: {"kind":"wifi|eth|none","label":"SSID"}.
    Net,
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
        Command::Hub { dump, snapshot, query, at } => {
            hub::run(&ctx, hub::Options { dump, snapshot, query, at_ms: at })?
        }
        Command::Open { id, args } => hub::open(&ctx, &id, &args)?,
        Command::Status { what: StatusWhat::Net } => cmd::status::run_net()?,
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
