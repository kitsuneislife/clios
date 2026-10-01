pragma ComponentBehavior: Bound

import QtQuick
import qs.core

// Indicador de volume/brilho: rótulo, barra de blocos e número. Sem ícone, sem moldura.
//
//   vol  ▮▮▮▮▮▮▮▯▯▯  70
Rectangle {
    id: root
    property string label: "vol"
    property real value: 0.5      // 0..1
    property bool muted: false
    property bool shown: false
    readonly property int blocks: 10

    implicitWidth: row.implicitWidth + Tokens.gapOut * 2
    implicitHeight: Tokens.barHeight + Tokens.gapIn
    color: Tokens.surface
    border.color: Tokens.line
    border.width: 1
    radius: Tokens.radius

    opacity: shown ? 1 : 0
    // entra subindo `travel` px, sai mais rápido do que entrou
    transform: Translate {
        y: root.shown ? 0 : Tokens.travel
        Behavior on y {
            enabled: Tokens.motionEnabled
            NumberAnimation { duration: root.shown ? Tokens.base : Tokens.exitMs(Tokens.base); easing.type: Easing.BezierSpline; easing.bezierCurve: root.shown ? Tokens.curveOut : Tokens.curveIn }
        }
    }
    Behavior on opacity {
        NumberAnimation { duration: Tokens.motionEnabled ? (root.shown ? Tokens.fast : Tokens.exitMs(Tokens.fast)) : 0; easing.type: Easing.BezierSpline; easing.bezierCurve: root.shown ? Tokens.curveOut : Tokens.curveIn }
    }

    Row {
        id: row
        anchors.centerIn: parent
        spacing: Tokens.gapOut
        Mono { text: root.label; color: Tokens.dim; width: implicitWidth }
        Row {
            anchors.verticalCenter: parent.verticalCenter
            spacing: 3
            Repeater {
                model: root.blocks
                delegate: Rectangle {
                    required property int index
                    width: 6
                    height: Tokens.text
                    radius: Tokens.radiusSmall / 2
                    readonly property bool on: !root.muted && (index + 0.5) / root.blocks <= root.value
                    color: on ? Tokens.accent : Tokens.line
                    Behavior on color { ColorAnimation { duration: Tokens.motionEnabled ? Tokens.instant : 0 } }
                }
            }
        }
        Mono {
            text: root.muted ? "mudo" : Math.round(root.value * 100)
            color: root.muted ? Tokens.mute : Tokens.fg
            // largura fixa: o número não empurra a barra quando passa de 99 para 100
            width: Tokens.text * 2.6
        }
    }
}
