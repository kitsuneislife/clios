import QtQuick
import Quickshell
import Quickshell.Io
import qs.core

// Observa theme.json e alimenta o singleton Tokens. Quando `clios theme apply` regrava o arquivo,
// as cores da shell inteira animam até os valores novos, sem reiniciar nada.
Scope {
    readonly property string stateHome: Quickshell.env("XDG_STATE_HOME") || (Quickshell.env("HOME") + "/.local/state")

    FileView {
        path: stateHome + "/clios/theme.json"
        watchChanges: true
        onFileChanged: reload()
        onLoaded: {
            try {
                Tokens.apply(JSON.parse(text()))
            } catch (e) {
                // Arquivo cortado no meio de uma escrita, ou de uma versão incompatível: mantém o que tem.
                console.warn("theme.json ilegível, mantendo o tema atual:", e)
            }
        }
    }
}
