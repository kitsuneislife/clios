import QtQuick
import Quickshell
import Quickshell.Io
import Quickshell.Wayland
import qs.core
import qs.components

// Papel de parede. `clios wallpaper` grava a escolha em wallpaper.json; a shell observa o arquivo e
// troca a imagem em fade. Sem arquivo ainda, fica a cor de fundo com a marca quieta no canto.
Scope {
    id: root
    readonly property string stateHome: Quickshell.env("XDG_STATE_HOME") || (Quickshell.env("HOME") + "/.local/state")
    property url source

    FileView {
        path: root.stateHome + "/clios/wallpaper.json"
        watchChanges: true
        onFileChanged: reload()
        onLoaded: {
            try {
                const p = JSON.parse(text())
                if (p.path) root.source = "file://" + p.path
            } catch (e) {
                console.warn("wallpaper.json ilegível, mantendo a imagem atual:", e)
            }
        }
    }

    Variants {
        model: Quickshell.screens

        PanelWindow {
            required property var modelData
            screen: modelData
            anchors { top: true; bottom: true; left: true; right: true }
            exclusionMode: ExclusionMode.Ignore
            WlrLayershell.layer: WlrLayer.Background
            WlrLayershell.namespace: "clios-bg"
            color: Tokens.bg
            Behavior on color { ColorAnimation { duration: Tokens.motionEnabled ? Tokens.slow : 0 } }

            WallpaperView {
                id: view
                anchors.fill: parent
                source: root.source
            }

            // Só enquanto não há imagem: o canto assinado, como na V0.1.
            Mark {
                visible: !view.ready
                anchors { right: parent.right; bottom: parent.bottom; rightMargin: 56; bottomMargin: 56 }
                size: 44
                body: Tokens.line
                cursor: Tokens.accentDim
                blink: true
            }
        }
    }
}
