import QtQuick
import Quickshell
import Quickshell.Wayland
import qs.core
import qs.components

// Papel de parede: o preto (ou o branco) e a marca, quieta, no canto. Sem imagem, sem daemon.
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

        Mark {
            anchors { right: parent.right; bottom: parent.bottom; rightMargin: 56; bottomMargin: 56 }
            size: 44
            body: Tokens.line
            cursor: Tokens.accentDim
            blink: true
        }
    }
}
