import QtQuick
import qs.core
import qs.components

// Cena de teste: os componentes visuais com dados de exemplo, no tema que o Python injetou.
Rectangle {
    id: root
    width: 1280
    height: 560
    color: Tokens.bg

    property string themeJson: "{}"
    property int active: 2
    Component.onCompleted: Tokens.apply(JSON.parse(themeJson))

    readonly property var sampleWorkspaces: [
        { id: 1, active: root.active === 1, occupied: true, urgent: false },
        { id: 2, active: root.active === 2, occupied: true, urgent: false },
        { id: 3, active: root.active === 3, occupied: true, urgent: false },
        { id: 4, active: root.active === 4, occupied: false, urgent: true },
        { id: 7, active: root.active === 7, occupied: false, urgent: false }
    ]

    Column {
        anchors { left: parent.left; right: parent.right; top: parent.top }
        spacing: 0

        BarView {
            id: bar
            width: parent.width
            workspaces: root.sampleWorkspaces
            title: "clios — hx crates/clios/src/hub/ui.rs"
            volume: 0.42
            netKind: "wifi"
            netLabel: "Casa 5G"
            battery: 0.87
            charging: false
            clock: "qua 1  14:32"
        }
    }

    Item {
        id: lower
        anchors { left: parent.left; right: parent.right; top: parent.top; topMargin: 80; bottom: parent.bottom }

        Column {
            anchors { left: parent.left; leftMargin: 48; top: parent.top; topMargin: 24 }
            spacing: 24
            OsdView { id: osd; shown: true; label: "vol"; value: 0.7 }
            OsdView { id: osdMuted; shown: true; label: "vol"; value: 0.7; muted: true }
            Mark { size: 96; blink: false }
        }

        Column {
            anchors { right: parent.right; rightMargin: 48; top: parent.top; topMargin: 24 }
            spacing: Tokens.gapIn
            ToastView {
                id: t1
                shown: true
                app: "Firefox"
                summary: "Download concluído"
                body: "clios-0.1.0-x86_64.iso (1,2 GB)"
                actions: [{ id: "open", text: "abrir" }, { id: "folder", text: "pasta" }]
            }
            ToastView { id: t2; shown: true; app: "sistema"; summary: "Bateria em 12%"; body: "Conecte o carregador."; urgent: true }
        }
    }
}
