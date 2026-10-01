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
    // liga-desliga ativos: só aparecem enquanto estão ligados
    property bool rec: false
    property bool caffeine: false
    property bool night: false
    property bool dnd: false

    signal workspaceActivated(int id)
    signal audioClicked
    signal netClicked
    signal batteryClicked
    signal clockClicked
    signal indicatorClicked(string name)

    implicitHeight: Tokens.barHeight
    // A barra flutua: superfície própria, canto arredondado e um filete de 1px. Sem sombra.
    color: Tokens.surface
    radius: Tokens.radius
    border.color: Tokens.line
    border.width: 1
    Behavior on color { ColorAnimation { duration: Tokens.motionEnabled ? Tokens.fast : 0 } }

    WorkspaceStrip {
        id: ws
        anchors { left: parent.left; leftMargin: Tokens.gapOut; verticalCenter: parent.verticalCenter }
        height: parent.height
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
        height: parent.height
        spacing: 0

        Segment {
            visible: root.rec
            text: "● gravando"
            color: Tokens.red
            onClicked: root.indicatorClicked("rec")
        }
        Segment {
            visible: root.dnd
            text: "silêncio"
            color: Tokens.accent
            onClicked: root.indicatorClicked("dnd")
        }
        Segment {
            visible: root.night
            text: "noturno"
            color: Tokens.accent
            onClicked: root.indicatorClicked("night")
        }
        Segment {
            visible: root.caffeine
            text: "café"
            color: Tokens.accent
            onClicked: root.indicatorClicked("caffeine")
        }
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
