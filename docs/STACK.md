# A stack

Para cada função há uma escolha só. Abaixo está o que foi escolhido, o que ficou de fora e o que a escolha custa. A regra de desempate foi a ordem dos pilares: leveza, depois limpeza, depois coerência.

As medidas citadas vêm de benchmarks públicos (links no fim), não foram refeitas aqui.

## Base

**Arch Linux, com archiso para a ISO.**
Hyprland e Quickshell andam rápido e o Arch os entrega logo; o AUR cobre as TUIs que ainda não têm pacote oficial (`ani-cli`); o `pacman` é simples de entender e de scriptar.

Descartados: NixOS, que daria reprodutibilidade de verdade, mas cobra a linguagem Nix e tende a ficar atrás do Hyprland em versão; Void e Alpine, mais leves em disco, mas com menos pacotes prontos do ecossistema Wayland (e musl, no caso do Alpine, que ainda pega alguns programas).
Custo: rolling release pede atualizar com atenção. O `bootstrap.sh` e os testes existem para tornar isso previsível.

Kernel `linux` padrão, `systemd-boot` (sem menu: `timeout 0`), zram como swap. O console de texto, até o login, usa a paleta do CLIOS pelos parâmetros `vt.default_*` do kernel (`clios theme cmdline`).

## Compositor: Hyprland

A configuração agora é Lua (desde a 0.55; o formato antigo está em desuso), o que permite escrever atalhos como dados e validar tudo com um interpretador. Tem curvas de mola nativas, que o design de movimento usa.

Descartados: **niri**, que é sólido e tem uma ideia boa de layout em rolagem, mas o Hyprland tem mais controle de animação e regras; **sway**, estável e minúsculo, mas sem animações.
Custo: Hyprland muda a configuração com frequência. Por isso há um validador contra o stub oficial da API (`tests/hypr`).

## Shell: Quickshell

Um processo só cobre barra, OSD, notificações e papel de parede, escrito em QML e com animação por dentro. A alternativa usual é Waybar + mako + swayosd + hyprpaper: quatro daemons, quatro formatos de configuração, nenhuma animação coerente.

Custo: puxa o Qt. É mais pesado que a Waybar sozinha, mas mais leve que as quatro peças somadas, e dá controle total do movimento.
O módulo de rede do Quickshell só fala com o NetworkManager. Como o wi-fi aqui é do `iwd`, a barra pergunta ao `clios status net` (Rust, lê `/sys`).

## Terminal: foot

Nativo de Wayland, o mais leve da lista, e com modo servidor: `foot --server` fica residente e cada janela é um `footclient`, que abre em milissegundos e quase sem memória extra. Imagens no yazi funcionam por sixel.

| terminal | RAM ociosa | observação |
|---|---|---|
| Alacritty | 20 a 35 MB | sem imagens inline |
| foot | sem número nas fontes citadas | citado como a escolha dos minimalistas de Wayland; em modo servidor, as janelas dividem um processo |
| Ghostty | 24 a 100 MB, conforme a fonte | o mais rápido em vazão |
| Kitty | 40 a 70 MB | mais recursos, mais peso |

Descartados: Ghostty e Kitty trazem abas, splits e protocolo gráfico próprio, que aqui seriam peso duplicado, porque o Hyprland já é o multiplexador.
Custo: sem splits no terminal. Para sessões remotas que sobrevivam a uma queda, dá para usar `tmux` ou `zellij` no servidor.

## Shell interativo: fish, starship, zoxide, fzf

fish tem destaque de sintaxe, autosugestão e completação úteis sem configurar nada. O prompt (starship, em Rust) é uma linha: diretório, git, cursor. `zoxide` e `fzf` cuidam de navegação e histórico.
Custo: fish não é compatível com POSIX. Scripts continuam em `bash` ou em Rust; o fish é para uso interativo.

## Editor: Helix

Um binário com LSP, tree-sitter, seleção múltipla e busca difusa já dentro. A configuração toda tem umas quarenta linhas, e nada para manter além dela.

Descartado: Neovim, que é mais poderoso e tem o maior ecossistema, mas pede escolher, configurar e atualizar dezenas de plugins. Para um desktop que prioriza leveza e limpeza, pesou mais não ter essa manutenção.
Custo: sem plugins. O modelo "selecione, depois aja" é diferente do Vim e tem curva de aprendizado se você vem de lá.

## Arquivos, git, monitor

- **yazi** (Rust, assíncrono, com prévia de imagem) em vez de ranger (mais lento) ou lf/nnn (mais austeros). A função `y` do fish leva o shell ao diretório onde você parou.
- **lazygit** em vez de gitui. O gitui é mais leve e em Rust, mas não faz rebase interativo, e isso pesa no uso diário.
- **btop** em vez de bottom. O btop tem o melhor uso de espaço e responde ao mouse.

## Rede, áudio, bluetooth

| função | escolha | alternativa | custo |
|---|---|---|---|
| wi-fi | `iwd` + `impala` | NetworkManager + `nmtui` | menos recursos de VPN e perfis corporativos |
| áudio | PipeWire + `wiremix` | PulseAudio, `pulsemixer` | |
| bluetooth | `bluez` + `bluetui` | `bluetoothctl` à mão | |

A barra mostra o volume pelo PipeWire diretamente, então o OSD aparece não importa quem mudou o volume (tecla, `wiremix`, um aplicativo).

## Mídia e comunicação

- **spotify_player** para o Spotify. Precisa de conta Premium.
- **mpv** para vídeo e áudio local, **imv** para imagens, **zathura** para PDF (gráfico, mas controlado só por teclado).
- **ani-cli** para anime (AUR). Depende de sites de terceiros e quebra quando eles mudam; costuma ser corrigido rápido a montante.
- **aerc** para e-mail, **newsboat** para RSS.

## O catálogo de apps de terminal

Mais de cinquenta apps curados em `config/clios/hub.toml`, em dez categorias, cada um com descrição, dica de uso e nível: *core* vem instalado, *extra* instala sob demanda pelo hub (`+`) ou por `clios apps install`. A escolha seguiu o mesmo critério do resto: um app por função, o que tem melhor UX no terminal, e que combine com o tema.

| função | app | por quê |
|---|---|---|
| arquivos | yazi, gdu, television | prévia de imagens, uso de disco, busca difusa |
| código | helix, lazygit, lazydocker, atac, rainfrog | edição, git, contêineres, API, banco |
| sistema | btop, wiremix, lnav, fend | monitor, áudio, logs, calculadora com unidades |
| rede | impala, bluetui, wavemon, bandwhich, trippy, sshs, termscp | wi-fi, bluetooth, sinal, tráfego, rota, SSH, transferência |
| mídia | spotify-player, kew, cava, ani-cli, lobster, manga-tui, ytfzf | música, visualizador, anime, filmes, mangá, YouTube |
| ler | newsboat, glow, presenterm, navi | feeds, markdown, apresentações, cheatsheets |
| conversar | aerc, weechat, iamb, nchat, toot | e-mail, IRC, Matrix, mensageiros, Mastodon |
| produtividade | calcurse, taskwarrior-tui, dijo | agenda, tarefas, hábitos |
| pacotes | `clios update`, pacseek | atualização com notícias do Arch, busca de pacotes |
| diversão | cbonsai, asciiquarium, cmatrix, genact | para olhar enquanto o resto compila |

Os nomes dos pacotes foram conferidos contra os repositórios do Arch e o AUR (`tests/tools/pkgcheck.py`). O instalador usa `paru` quando existe (ele resolve oficial e AUR) e cai para `pacman`. O `bootstrap.sh` compila o paru sozinho.

Também entram no shell: **atuin** (Ctrl+R com busca difusa e contexto, tudo local) e **tldr**.

## Pequenas ferramentas

`clios update` (lê o feed de notícias do Arch antes de atualizar e avisa de intervenção manual e de `.pacnew`), `caffeine` (systemd-inhibit), `night` (hyprsunset), `dnd` (a shell observa um arquivo), `rec` (wf-recorder), `pick` (hyprpicker), `power` (power-profiles-daemon) e `saver`, a proteção de tela própria. Cada uma é um comando, um atalho e uma ação no hub, e o que fica ligado aparece na barra.

## Login, bloqueio, idle

- **greetd + tuigreet**: login em terminal, sem toolkit gráfico, nas cores do console do CLIOS. Um login próprio em Rust, com a marca e a animação do resto, está no roteiro.
- **hyprlock + hypridle**: maduros e testados. Uma tela de bloqueio em QML seria mais coerente, mas bloqueio de sessão é o último lugar para experimentar.

## A exceção gráfica: Firefox

A regra "tudo no terminal" tem um limite: sites que exigem um navegador de verdade, vídeo com DRM, apps web. O CLIOS reconhece isso em vez de fingir: o Firefox é o único programa gráfico de uso diário, aberto por `SUPER + W` ou pelo hub. Alternativa mais fiel à filosofia: qutebrowser (teclado primeiro, mas puxa o QtWebEngine).

## Fonte

**Geist Mono** (via `otf-geist-mono-nerd`, no `extra`). Uma família para tudo. A variante Nerd Font existe para os glifos de caixa e símbolos que as TUIs usam; a interface do CLIOS não usa ícones.

## O que ficou de fora

- **Plymouth**: uma animação de boot é decoração, e atrasa. O boot é `systemd-boot` direto para o login, com o console já na paleta certa.
- **tmux** como padrão: o Hyprland já divide a tela.
- **Gerenciador de snapshots**: btrfs com snapper é uma boa escolha, mas depende de como você particiona. Fica como recomendação, não como imposição.
- **Instalador próprio**: por enquanto `archinstall` + `scripts/bootstrap.sh`.

## Fontes das medidas

- [Benchmarks de terminais Wayland](https://github.com/isvizen/terminal-benchmark)
- [Alacritty vs Kitty, 2026](https://botmonster.com/self-hosting/alacritty-vs-kitty-best-high-performance-linux-terminal-2026/)
- [Comparação de terminais, 2026](https://dashen-tech.com/en/dev-tools/terminal-emulator-comparison-2026/)
- [Yazi](https://blog.starmorph.com/blog/yazi-terminal-file-manager-guide)
- [Configuração Lua do Hyprland](https://wiki.hypr.land/configuring/)
