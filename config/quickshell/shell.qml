// CLIOS · shell. Cada módulo é uma peça independente; comente uma linha para desligar a peça.
//
// Configuração: tudo vem de ~/.local/state/clios/theme.json (gerado por `clios theme apply`).
// A shell recarrega o tema ao vivo; nada aqui guarda cor.

import Quickshell
import qs.modules

ShellRoot {
    ThemeFile {}
    Wallpaper {}
    Bar {}
    Osd {}
    Notifications {}
}
