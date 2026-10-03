# A stack

Para cada função há uma escolha só. Abaixo está o que foi escolhido, o que ficou de fora e o que a escolha custa. A regra de desempate foi a ordem dos pilares: leveza, depois limpeza, depois coerência.

As medidas citadas vêm de benchmarks públicos (links no fim), não foram refeitas aqui.

## Base

**Arch Linux, com archiso para a ISO.**
Hyprland e Quickshell andam rápido e o Arch os entrega logo; o AUR cobre as TUIs que ainda não têm pacote oficial (`ani-cli`); o `pacman` é simples de entender e de scriptar.

Descartados: NixOS, que daria reprodutibilidade de verdade, mas cobra a linguagem Nix e tende a ficar atrás do Hyprland em versão; Void e Alpine, mais leves em disco, mas com menos pacotes prontos do ecossistema Wayland (e musl, no caso do Alpine, que ainda pega alguns programas).
Custo: rolling release pede atualizar com atenção. O `bootstrap.sh` e os testes existem para tornar isso previsível.

Kernel `linux` padrão, zram como swap, e de preferência btrfs com os subvolumes padrão do `archinstall` (é o que liga as fotografias do sistema, abaixo). Carregador de boot: o `systemd-boot` (sem menu: `timeout 0`) funciona; o limine é o recomendado quando há btrfs, porque lista as fotografias no menu de boot. O console de texto, até o login, usa a paleta do CLIOS pelos parâmetros `vt.default_*` do kernel (`clios theme cmdline`).

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

fish tem destaque de sintaxe, autosugestão e completação úteis sem configurar nada. O prompt (starship, em Rust) tem três estilos, e o padrão é uma linha: diretório, git, cursor. `zoxide` e `fzf` cuidam de navegação e histórico.
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

44 apps curados em `config/clios/hub.toml`, em dez categorias, cada um com descrição, dica de uso e nível: *core* vem instalado, *extra* instala sob demanda pelo hub (`+`) ou por `clios apps install`. A escolha seguiu o mesmo critério do resto: o que tem melhor UX no terminal e combina com o tema.

**Um trabalho, um app.** Cada entrada declara um `job`, e o teste `one_job_one_app` reprova o catálogo se dois apps declararem o mesmo. Se aparece um segundo app para um trabalho que já tem dono, ele precisa ser melhor e tomar o lugar do primeiro. Na V0.3 a regra tirou quatro:

| saiu | ficou | por quê |
|---|---|---|
| television | fzf | o fzf já está ligado ao fish (`Ctrl+T`, `Alt+C`) e é dependência de outros apps do catálogo; dois buscadores difusos são um a mais |
| wavemon | impala | o impala já mostra a força do sinal na lista de redes |
| navi | tealdeer (`tldr`) | os dois respondem "como uso este comando"; o tldr é menor, vem instalado e tem a abreviação `?` |
| lobster | mpv e Firefox | filmes e séries por scraping quebram toda hora, e o mpv já toca o que você tiver |

Os pares que parecem repetidos e ficaram têm trabalhos diferentes: `spotify-player` (streaming) e `kew` (a sua biblioteca local); `weechat` (IRC), `iamb` (Matrix), `nchat` (Telegram e WhatsApp) e `toot` (Mastodon) falam protocolos que não se traduzem entre si; `calcurse` (agenda), `taskwarrior-tui` (tarefas) e `dijo` (hábitos).

| categoria | apps |
|---|---|
| arquivos | yazi, gdu |
| código | helix, lazygit, lazydocker, atac, rainfrog |
| sistema | btop, wiremix, lnav, fend, e os do CLIOS (guia, central, notificações, papel de parede, resumo) |
| rede | impala, bluetui, bandwhich, trippy, sshs, termscp |
| mídia | spotify-player, kew, ani-cli, manga-tui, ytfzf |
| ler | newsboat, glow, presenterm |
| conversar | aerc, weechat, iamb, nchat, toot |
| produtividade | calcurse, taskwarrior-tui, dijo |
| pacotes | pacseek, e `clios update` e `clios self-update` |
| diversão | cbonsai, asciiquarium, cmatrix, genact, lavat, csakura, pipes.sh, tty-clock, cava |

Os nomes dos pacotes foram conferidos contra os repositórios do Arch e o AUR (`tests/tools/pkgcheck.py`). O instalador usa `paru` quando existe (ele resolve oficial e AUR) e cai para `pacman`. O `bootstrap.sh` compila o paru sozinho.

Também entram no shell: **atuin** (Ctrl+R com busca difusa e contexto, tudo local) e **tldr**.

## O terminal por dentro

**Prompt.** O starship tem três estilos, e `clios prompt` troca na hora: *minimal* (uma linha, o padrão), *dev* (duas linhas, com as versões das linguagens quando o projeto usa) e *zen* (só a seta, com pasta e branch no canto direito). O fish usa o prompt transitório do starship: depois que o comando roda, o prompt antigo vira só a seta, e o histórico na tela fica limpo.

**Fastfetch.** O `clios fetch` chama o fastfetch com uma config gerada do tema (acento nos rótulos, a marca do CLIOS como logo, redesenhada a cada troca de acento). Sem o fastfetch instalado, um desenho embutido cobre o essencial. Não há dois comandos para a mesma coisa: `ff` e `clios fetch` são a mesma entrada.

**Quando o terminal se apresenta.** `clios greet` roda quando o fish abre e decide por uma função pura e testada. O primeiro terminal depois de ligar mostra o resumo do sistema. O primeiro terminal de uma workspace vazia mostra duas linhas: a saudação e uma dica do guia. O resto do tempo, nada. Terminais do scratchpad, janelas flutuantes e apps de terminal nunca são apresentados, e `ssh` também não. O que já foi mostrado fica em `$XDG_RUNTIME_DIR`, que zera a cada boot. `clios greet --mode boot` deixa só a primeira, e `--mode off` desliga.

**Brinquedos.** Cada brinquedo do catálogo (`diversao`) que sabe rodar sozinho tem um campo `saver`, com `{color}` e `{n}` no lugar da cor: o CLIOS acha o matiz do seu acento e entrega a cor ANSI mais perto dele (a maioria dos brinquedos só fala as oito cores ANSI). A proteção de tela (`clios saver`) reveza entre a marca e os brinquedos instalados, ou fica numa cena só, ou desliga (`clios saver set`). O hypridle fecha a janela ao primeiro sinal de vida. `clios play` roda um aqui mesmo, e `SUPER + Z` abre um ao acaso.

## Sessões

**Terminais com ficha.** O `SUPER + Enter` chama `clios term`, que abre o footclient com uma ficha no título inicial (`clios-term:<ficha>`) e na variável `CLIOS_TERM`. O título muda logo, mas o Hyprland guarda o inicial, e é assim que uma janela leva à sua ficha. O fish escreve nela a pasta e o comando em execução, a cada prompt, só com builtins (um `printf` para um arquivo em `$XDG_RUNTIME_DIR`): nenhum processo novo por prompt.

**A sessão.** A cada minuto, `clios session save` cruza as janelas do `hyprctl clients` com as fichas. No login, `clios session restore` reabre cada janela pela regra do `exec` do Hyprland, na workspace dela e sem roubar o foco. Os terminais reabertos usam o `foot` sem servidor: a regra do `exec` acha a janela pelo processo, e no modo servidor todas as janelas são do mesmo processo. Só voltam a rodar programas interativos conhecidos (editores, `less`, `man` e os binários do catálogo) e nunca uma linha com pipe, redirecionamento ou substituição.

Descartados: as ferramentas genéricas de sessão do Hyprland (hyprflow, hypr-session-restore e afins), que reabrem o app mas não sabem o que havia dentro de um terminal; o tmux-resurrect, que exige o tmux.

**Comando longo.** O fish mede cada comando; passando de 15 segundos, chama `clios term done`, que só notifica se a janela daquele terminal não estiver em foco.

## Fotografias do sistema

**snapper + snap-pac**, ligados pelo bootstrap quando a raiz é btrfs: cada transação do pacman ganha uma fotografia antes e outra depois, sem fotografias de hora em hora (12 guardadas, 6 importantes). O seu usuário está em `ALLOW_USERS`, então listar e comparar não pedem sudo. Com o limine, o **limine-snapper-sync** (AUR) põe as fotografias no menu de boot.

`clios snap undo` usa o `snapper undochange`, que devolve os arquivos da raiz com o sistema rodando. O kernel mora no /boot (FAT), fora das fotografias, então uma transação que trocou o kernel é recusada: desfazer deixaria módulos e imagem desencontrados. Para esse caso, o caminho é o menu de boot ou o cache do pacman.

Descartados: o Timeshift (pensado para a interface gráfica e para o rsync), e fotografias de hora em hora (enchem o disco e quase nunca são as que você quer).

## O diálogo de arquivo

**xdg-desktop-portal-termfilechooser** (AUR, o fork mantido do hunkyburrito). Quando um app pede um arquivo pelo portal, ele chama `clios-filechooser`, que é o próprio binário do clios por outro nome. Abrir usa o yazi como seletor (`--chooser-file`); salvar mostra um campo de nome com a pasta sugerida, e `tab` abre o yazi para trocar de pasta. O `hyprland-portals.conf` põe o termfilechooser na frente e o GTK como reserva, e uma política em `/etc/firefox/policies` faz o Firefox usar o portal.

## Pequenas ferramentas

`clios update` (lê o feed de notícias do Arch antes de atualizar e avisa de intervenção manual e de `.pacnew`), `caffeine` (systemd-inhibit), `night` (hyprsunset), `dnd` (a shell observa um arquivo), `focus` (um bloco de foco: liga o não perturbe, a barra conta o tempo e avisa no fim), `rec` (wf-recorder), `pick` (hyprpicker), `ocr` (grim, slurp e tesseract: copia o texto de uma região da tela), `notifs` (o histórico de notificações, inclusive as que o silêncio engoliu), `power` (power-profiles-daemon), `keys` (os atalhos no terminal), `battery` (saúde, ciclos e um limite de carga que o systemd-tmpfiles reaplica a cada boot) e `saver`, a proteção de tela própria. O `night` também agenda (`clios night auto 20:30-06:45`): o CLIOS escreve dois perfis no `hyprsunset.conf` e o hyprsunset troca sozinho. No hub, `=` faz contas pelo fend. Cada uma é um comando, um atalho e uma ação no hub, e o que fica ligado aparece na barra.

## Discos, arquivos, hora e firewall

- **udiskie** monta pendrives e HDs externos sozinho e avisa; `udiskie-umount --all` ejeta tudo. Sem isso, um terminal puro não vê o pendrive.
- **mimeapps**: `xdg-open`, o `open` do fish e os links do terminal abrem texto no helix, pastas no yazi e e-mail no aerc (por `.desktop` do CLIOS em `config/applications/`), PDF no zathura, imagens no imv e vídeo e áudio no mpv. Um teste confere que cada associação aponta para um `.desktop` que existe.
- **systemd-timesyncd** acerta a hora sozinho.
- **nftables**: entrada fechada, saída livre, numa tabela própria (`inet clios`) que não toca nas regras do Docker. Abrir uma porta é uma linha em `/etc/nftables.conf`.
- **noto-fonts-cjk**: japonês, chinês e coreano no terminal e no navegador (os títulos de anime e mangá do catálogo precisam).

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
- **Instalador próprio**: por enquanto `archinstall` + `scripts/bootstrap.sh`.

## Fontes das medidas

- [Benchmarks de terminais Wayland](https://github.com/isvizen/terminal-benchmark)
- [Alacritty vs Kitty, 2026](https://botmonster.com/self-hosting/alacritty-vs-kitty-best-high-performance-linux-terminal-2026/)
- [Comparação de terminais, 2026](https://dashen-tech.com/en/dev-tools/terminal-emulator-comparison-2026/)
- [Yazi](https://blog.starmorph.com/blog/yazi-terminal-file-manager-guide)
- [Configuração Lua do Hyprland](https://wiki.hypr.land/configuring/)
