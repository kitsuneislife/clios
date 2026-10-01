# Mudanças

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
