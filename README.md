<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="brand/wordmark.svg">
    <img src="brand/wordmark-on-light.svg" height="72" alt="clios">
  </picture>
</p>

Um desktop Linux feito para o terminal. Arch, Hyprland e Quickshell, com uma regra: o que dá para fazer num terminal é feito num terminal. O resto do sistema é mínimo o bastante para sumir.

Preto no branco e branco no preto, mais um acento que você escolhe. Uma fonte só. Movimento curto, que respeita quem não quer movimento.

![barra, OSD e notificações no tema escuro](docs/img/shell-dark.png)

## O que tem aqui

- **`clios`**, um binário em Rust, com os comandos do sistema: `theme`, `hub`, `open`, `sync`, `shot`, `status`, `motion`, `doctor`.
- **Tokens de design** (`tokens/tokens.toml`) que controlam cor, movimento e tipografia. Hyprland, Quickshell, foot, helix, fish, btop, yazi, lazygit, mpv e a tela de bloqueio leem os mesmos valores. Trocar o acento ou o modo muda tudo junto, e os terminais abertos mudam na hora.
- **O hub**, um lançador que roda dentro de um terminal (`SUPER + espaço`): apps, TUIs, ações, janelas abertas, atalhos e histórico da área de transferência numa busca só.
- **Configuração do Hyprland em Lua**, com todos os atalhos documentados (`SUPER + /` lista todos), e **uma shell em Quickshell** (barra, OSD, notificações, papel de parede).
- **Instalação**: `scripts/bootstrap.sh` sobre um Arch mínimo, e um perfil de ISO ao vivo.

![o hub](docs/img/hub.png)

## Estado atual

Leia isto antes de instalar.

**Testado** (138 testes de Rust, mais os validadores abaixo, tudo rodando no CI):

- Os tokens e o contraste: todo acento e toda cor de texto passa de 4.5:1 nos dois modos, e o build quebra se isso piorar.
- Os templates de cada app renderizam em 48 combinações (modo × acento × nível de movimento), e JSON e TOML saem válidos.
- O config Lua do Hyprland roda num interpretador contra o stub oficial da API do Hyprland 0.56. Funções, nome e tipo de cada opção, folhas de animação, efeitos de regra e conflitos de atalho são conferidos. O próprio validador é testado com 18 configurações erradas que ele precisa reprovar.
- Os componentes visuais da shell renderizam fora do Quickshell (PySide6) sem nenhum aviso do QML, nos dois modos.
- O hub: busca, histórico, teclado, mouse, confirmação em dois passos e o desenho de cada quadro da animação.
- Os scripts passam no `shellcheck`, e o `bootstrap.sh` tem `--dry-run`.

**Nunca rodou de verdade**:

- Nenhuma peça foi executada dentro de uma sessão do Hyprland, nem o Quickshell (os módulos que falam com PipeWire, UPower, notificações e Hyprland só foram conferidos contra o código-fonte do Quickshell e com `qmllint`).
- Os nomes dos pacotes do Arch foram conferidos por pesquisa, não por instalação. O bootstrap pula e avisa os que não existirem.
- A ISO não foi montada. O perfil e o `build.sh` partem do `releng` oficial, mas esperam ajustes na primeira execução.
- Nada foi testado em hardware.

Se você tentar e algo quebrar, o `clios doctor` diz o que falta, e uma issue com a saída dele ajuda muito.

## Instalação

Em um Arch já instalado (`archinstall`, perfil *minimal*, `systemd-boot`):

```sh
git clone https://github.com/kitsuneislife/clios
cd clios
./scripts/bootstrap.sh --dry-run     # veja o que vai acontecer
./scripts/bootstrap.sh               # instala pacotes, compila o clios, liga os dotfiles, ativa serviços
```

O script é idempotente. Seus arquivos de configuração que já existirem vão para `*.clios-bak` antes de serem substituídos. Para o console de texto (boot e TTY) usar a paleta do CLIOS, rode com `--kernel-cmdline`.

Para experimentar numa VM sem instalar, veja [`iso/`](iso/README.md).

## Uso

| | |
|---|---|
| `SUPER + espaço` | o hub |
| `SUPER + /` | todos os atalhos |
| `SUPER + F1` / `F2` / `F3` | tema claro ou escuro / próximo acento / nível de movimento |
| `SUPER + h j k l` | foco; com `shift`, move a janela |
| `SUPER + 1…0` | workspaces |
| `SUPER + E / G / A / I` | arquivos / git / áudio / rede |
| `Print` | captura de região (salva, copia e avisa) |

A lista completa, gerada do próprio config, está em [`docs/KEYS.md`](docs/KEYS.md).

```sh
clios theme set --mode light --accent azure
clios theme set --accent "#7CFF00"        # qualquer cor; é ajustada para ter contraste
clios motion reduced                      # só fades curtos
clios doctor                              # o que falta no sistema
```

Para mudar o catálogo do hub (as TUIs que aparecem), copie `config/clios/hub.toml` para `~/.config/clios/hub.toml`. Monitores e teclado ficam em `~/.config/hypr/user.lua` (há um `user.lua.example`).

## Como as peças se ligam

```
tokens/tokens.toml ──► clios theme apply ──┬─► ~/.config/hypr/theme.lua       (Hyprland: cores, curvas, molas)
   cor · movimento                         ├─► ~/.local/state/clios/theme.json (Quickshell lê ao vivo)
   tipografia · grade                      ├─► foot, helix, fish, btop, yazi, lazygit, mpv, hyprlock
                                           └─► OSC nos terminais abertos       (mudam sem reiniciar)

config/hypr/conf/binds.lua ──► hyprctl binds ──► hub (`?`) e docs/KEYS.md
config/clios/hub.toml ──────► hub  e  `clios open <id>`  ◄── atalhos do Hyprland
```

Dois detalhes que mudam o dia a dia: os atalhos de TUI chamam `clios open <id>`, então o catálogo do hub é a única fonte dos comandos; e a documentação de atalhos sai do próprio Lua, então não envelhece.

## Estrutura

```
tokens/        a fonte única de design
templates/     um template (minijinja) por app, e o manifesto que os liga
config/        dotfiles estáticos, ligados por symlink em ~/.config
seed/          arquivos que o app reescreve (btop): copiados uma vez
system/        arquivos de /etc (greetd, iwd, zram, sysctl)
crates/        clios-core (tokens, tema, sync) e clios (a CLI e o hub)
brand/         marca e wordmark em SVG, e o script que os gera
iso/           perfil da ISO ao vivo
scripts/       bootstrap.sh
tests/         validadores de Hyprland (Lua) e de Quickshell (QML), e o runner
docs/          STACK.md, DESIGN.md, KEYS.md
```

## Desenvolvimento

```sh
pip install -r tests/requirements.txt
./tests/run.sh
```

O runner executa `rustfmt`, `clippy`, os testes de Rust, os validadores de Lua e de QML, `shellcheck` e a sintaxe do fish. Etapas cujas ferramentas faltam são puladas com aviso. As imagens deste README saem de `clios hub --snapshot` e de `tests/qml/render.py`.

## Documentação

- [`docs/STACK.md`](docs/STACK.md): cada escolha, o que ficou de fora e o que a escolha custa.
- [`docs/DESIGN.md`](docs/DESIGN.md): a identidade: marca, cor, tipografia, forma e movimento.
- [`docs/KEYS.md`](docs/KEYS.md): atalhos.
- [`docs/identidade.html`](docs/identidade.html): a identidade ao vivo. Abra no navegador, troque modo, acento e movimento, e use o hub dentro do desktop ilustrado.

## Roteiro

- Um login em Rust (`greetd` + ratatui) com a marca e a animação do resto, no lugar do tuigreet.
- Um instalador, para não depender do `archinstall`.
- Papel de parede e tela de bloqueio opcionalmente animados pelos mesmos tokens.
- Tema para o Firefox (`userChrome.css` gerado dos tokens).

## Licença

MIT. A Geist Mono é distribuída pela Vercel sob a OFL; o stub da API Lua em `tests/hypr/` é do Hyprland (BSD-3-Clause).
