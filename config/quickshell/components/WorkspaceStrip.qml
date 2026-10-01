pragma ComponentBehavior: Bound

import QtQuick
import qs.core

// Números de workspace. Só aparecem as ocupadas e a ativa. O bloco do cursor (acento) desliza
// de um número a outro: a marca do CLIOS em movimento.
Item {
    id: root
    // [{ id: 1, active: true, occupied: true, urgent: false }, ...]
    property var workspaces: []
    signal activated(int id)

    readonly property var shown: workspaces.filter(w => w.active || w.occupied || w.urgent)
    readonly property int cell: Tokens.textSmall * 2 + Tokens.gapIn
    implicitWidth: Math.max(shown.length, 1) * cell
    implicitHeight: parent ? parent.height : Tokens.barHeight

    readonly property int activeIndex: {
        for (let i = 0; i < shown.length; i++) if (shown[i].active) return i
        return -1
    }

    // O cursor: um bloco do tamanho da célula, sob o número ativo.
    Rectangle {
        id: cursor
        visible: root.activeIndex >= 0
        x: Math.max(root.activeIndex, 0) * root.cell
        width: root.cell
        height: Tokens.barHeight - Tokens.gapIn * 2
        anchors.verticalCenter: parent.verticalCenter
        radius: Tokens.radiusSmall
        color: Tokens.accent
        Behavior on x {
            enabled: Tokens.motionEnabled
            NumberAnimation { duration: Tokens.fast; easing.type: Easing.BezierSpline; easing.bezierCurve: Tokens.curveOut }
        }
        Behavior on color { ColorAnimation { duration: Tokens.motionEnabled ? Tokens.fast : 0 } }
    }

    Row {
        anchors.fill: parent
        Repeater {
            model: root.shown
            delegate: Item {
                id: cellItem
                required property var modelData
                width: root.cell
                height: root.height
                Mono {
                    anchors.centerIn: parent
                    text: cellItem.modelData.id
                    small: true
                    font.bold: cellItem.modelData.active
                    color: cellItem.modelData.active ? Tokens.accentInk : (cellItem.modelData.urgent ? Tokens.red : Tokens.dim)
                }
                MouseArea {
                    anchors.fill: parent
                    cursorShape: Qt.PointingHandCursor
                    onClicked: root.activated(cellItem.modelData.id)
                }
            }
        }
    }
}
