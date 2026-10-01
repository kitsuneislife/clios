import QtQuick
import qs.core

// A barra, só visual. Recebe dados prontos por propriedade e devolve cliques por sinal,
// então pode ser desenhada fora do Quickshell (é assim que os testes a renderizam).
//
//   1  2  3        título da janela ativa              vol 42  wifi  87%  14:32
Rectangle {
    id: root
    property var workspaces: []
    property string title: ""
    property real volume: 0.0          // 0..1
    property bool muted: false
    property bool audioKnown: true
    property string netKind: "none"    // wifi | eth | none
    property string netLabel: ""
    property real battery: -1          // 0..1, -1 sem bateria
    property bool charging: false
    property string clock: ""

    signal workspaceActivated(int id)
    signal audioClicked
    signal netClicked
    signal batteryClicked
    signal clockClicked

    implicitHeight: Tokens.barHeight
    color: Tokens.bg
    Behavior on color { ColorAnimation { duration: Tokens.motionEnabled ? Tokens.fast : 0 } }

    // filete inferior de 1px: a barra se separa por linha, não por sombra
    Rectangle {
        anchors { left: parent.left; right: parent.right; bottom: parent.bottom }
        height: 1
        color: Tokens.line
    }

    WorkspaceStrip {
        id: ws
        anchors { left: parent.left; leftMargin: Tokens.gapOut; verticalCenter: parent.verticalCenter }
        height: parent.height - 1
        workspaces: root.workspaces
        onActivated: id => root.workspaceActivated(id)
    }

    Mono {
        id: titleLabel
        anchors { left: ws.right; leftMargin: Tokens.gapOut * 2; right: status.left; rightMargin: Tokens.gapOut * 2; verticalCenter: parent.verticalCenter }
        text: root.title
        color: Tokens.mute
        elide: Text.ElideRight
        small: true
        Behavior on text { enabled: false }
    }

    Row {
        id: status
        anchors { right: parent.right; rightMargin: Tokens.gapOut - Tokens.gapIn / 2; verticalCenter: parent.verticalCenter }
        height: parent.height - 1
        spacing: 0

        Segment {
            visible: root.audioKnown
            text: root.muted ? "mudo" : "vol " + Math.round(root.volume * 100)
            color: root.muted ? Tokens.mute : Tokens.dim
            onClicked: root.audioClicked()
        }
        Segment {
            text: root.netKind === "none" ? "offline" : (root.netLabel !== "" ? root.netLabel : root.netKind)
            color: root.netKind === "none" ? Tokens.mute : Tokens.dim
            onClicked: root.netClicked()
        }
        Segment {
            visible: root.battery >= 0
            text: (root.charging ? "+" : "") + Math.round(root.battery * 100) + "%"
            urgent: root.battery >= 0 && root.battery <= 0.15 && !root.charging
            color: Tokens.dim
            onClicked: root.batteryClicked()
        }
        Segment {
            text: root.clock
            color: Tokens.fg
            onClicked: root.clockClicked()
        }
    }
}
