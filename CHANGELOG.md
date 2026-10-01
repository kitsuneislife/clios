# Mudanças

## 0.3.0

**Terminal**

- Prompt em três estilos (`clios prompt`): *minimal*, *dev* (duas linhas, com as versões das linguagens do projeto) e *zen* (só a seta). O fish usa o prompt transitório: depois que o comando roda, o prompt antigo vira só a seta.
- `clios fetch` agora usa o fastfetch, com uma config gerada do tema e a marca do CLIOS como logo. Sem o fastfetch, o desenho embutido continua valendo.
- `clios greet`: o terminal se apresenta, mas só quando faz sentido. O resumo do sistema aparece no primeiro terminal depois de ligar, e duas linhas (saudação e uma dica) na primeira vez que uma workspace vazia recebe um terminal. `--mode boot` deixa só a primeira, `--mode off` desliga; também está na central do sistema.
- `selection-target=both` no foot: o que você seleciona vai para a área de transferência.

**Brinquedos**

- Entram lavat, csakura, pipes.sh e tty-clock; o cava passa para a categoria diversão. Todos aceitam a cor do seu acento (o matiz vira a cor ANSI mais perto).
- A proteção de tela reveza entre a marca e os brinquedos instalados. `clios saver set auto|marca|off|<brinquedo>`, `clios saver list` e `clios saver stop`; também na central.
- `clios play` roda um brinquedo aqui mesmo; `SUPER + Z` abre um ao acaso.

**Catálogo**

- Cada app declara o `job` que faz, e um teste reprova dois apps com o mesmo. Saem television (o fzf já cobre), wavemon (o impala mostra o sinal), navi (o tldr cobre) e lobster (o mpv e o Firefox cobrem). Dois itens novos do CLIOS entram: notificações e atualizar o clios.
- `clios apps info` mostra o trabalho de cada app.

**Atalhos**

- `SUPER + shift + S` captura uma região (o mesmo que `Print`). O envio ao scratchpad passou para `SUPER + ctrl + S`.
- `SUPER + Q` fecha a janela e `SUPER + shift + 1…0` manda a janela para a workspace: já existiam desde a 0.1, agora aparecem na tabela do README.
- Novos: `SUPER + X` (foco), `SUPER + shift + N` (histórico de notificações), `SUPER + shift + T` (OCR), `SUPER + Z` (brinquedo).

**Ferramentas novas**

- `clios focus`: um bloco de 25 minutos (ou o que você disser) com o não perturbe ligado, contagem na barra e um aviso no fim. O silêncio volta ao que era antes.
- `clios ocr`: seleciona uma região da tela e copia o texto dela (tesseract, português e inglês).
- `clios notifs`: o histórico de notificações, inclusive as que o não perturbe silenciou. A shell registra cada uma.
- `clios keys`: os atalhos no terminal, com filtro.
- `clios self-update`: puxa o repositório, recompila, reinstala e sincroniza.
- `clios completions fish|bash|zsh`, com os ids do catálogo no fish; o bootstrap instala.

**Buracos fechados**

- Pendrives e HDs externos montam sozinhos (udiskie).
- `xdg-open` e o `open` do fish abrem texto no helix, pastas no yazi e e-mail no aerc; PDF, imagem, vídeo e áudio têm app padrão (`config/mimeapps.list`, `config/applications/`).
- A hora se acerta sozinha (systemd-timesyncd) e há um firewall de entrada fechada numa tabela própria do nftables.
- Aviso de bateria fraca (20% e 10%) na shell.
- Fontes CJK (japonês, chinês, coreano) para os títulos de anime e mangá.
- Os arquivos de `config/applications/` vão para `~/.local/share/applications`; o resto continua em `~/.config`.

**Site e testes**

- `site/`: a página do projeto, para o GitHub Pages. O catálogo, os atalhos e este changelog entram nela por um script, então ela não envelhece.
- 277 testes de Rust. O runner confere os configs gerados contra o starship, o fastfetch e o nft de verdade, quando instalados.
- `tests/tools/termshot.py` roda um programa de terminal num pty e grava a tela como ANSI: é como saem as imagens do fastfetch e dos brinquedos.

**Ainda não verificado**

- Atalhos para mover foco e janelas entre monitores ficaram de fora: a API de monitor do Hyprland em Lua não pôde ser conferida aqui.
- Nada disto rodou numa sessão real do Hyprland e do Quickshell (a barra, o aviso de bateria e o registro de notificações usam serviços do Quickshell que só foram conferidos no código-fonte).

## 0.2.0

**Visual**

- Cantos levemente arredondados em tudo: janelas, barra (agora flutuante), cartões, OSD, o hub (seleção em pílula) e a marca. Raios nos tokens (`radius = 8`, `radius_small = 4`, `rounding_power = 3`).
- A marca desenhada com cantos suaves, com o mesmo contorno no SVG, no QML e no terminal; um teste confere que são idênticos.

**Boas-vindas e central**

- `clios welcome`: primeiros passos ao vivo, atalhos curados, catálogo de apps, central do sistema, dicas e sobre. Abre no primeiro login; `SUPER + F10` depois. A central (`SUPER + F9`) muda modo, acento, movimento, papel de parede, energia, café, noturno e não perturbe na hora.
- `clios fetch`: resumo do sistema com a marca em meio-bloco.

**Papel de parede**

- Oito estilos procedurais que seguem o modo e o acento, em qualquer resolução. Seletor com prévia (`clios wallpaper`, `SUPER + F4`), imagens suas, fade entre imagens na shell e o mesmo fundo no hyprlock.

**Apps**

- Catálogo curado com mais de cinquenta apps de terminal em dez categorias (descrição, dica, nível core ou extra). O hub instala o que falta com `+`; `clios apps list|info|install`. O `bootstrap.sh` compila o `paru`, aceita `--extras` e liga os timers de manutenção (paccache, reflector, pkgfile).
- Temas gerados para cava, lazydocker e atuin; Ctrl+R com atuin; `tldr`.

**Ferramentas**

- `clios update` (notícias do Arch antes, intervenção manual, `.pacnew`), `caffeine`, `night`, `dnd`, `rec`, `pick`, `power` e `saver` (a proteção de tela da marca). A barra mostra o que está ligado; `clios status all` alimenta a barra.
- Atalhos novos: `SUPER + C N D P U`, `SUPER + shift + R`, `SUPER + F4 F9 F10`.

**Testes**

- 234 testes de Rust. Novos validadores: os atalhos do guia contra o Lua, o contorno da marca QML contra o SVG, o papel de parede no harness de QML, e listas de pacotes contra o catálogo.
- `tests/tools/shots.sh` regenera todas as imagens da documentação.

## 0.1.0

Primeira versão: tokens de design, `clios` (theme, hub, open, sync, shot, status, motion, doctor), configuração do Hyprland em Lua, shell em Quickshell, bootstrap e perfil de ISO.
