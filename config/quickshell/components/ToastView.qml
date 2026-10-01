pragma ComponentBehavior: Bound

import QtQuick
import qs.core

// Uma notificação. Faixa lateral no acento (ou vermelha se urgente), texto direto.
Rectangle {
    id: root
    property string app: ""
    property string summary: ""
    property string body: ""
    property bool urgent: false
    property var actions: []        // [{ id, text }]
    property bool shown: false
    signal dismissed
    signal actionInvoked(var id)

    implicitWidth: 360
    implicitHeight: col.implicitHeight + Tokens.gapOut * 2
    color: Tokens.surface
    border.color: urgent ? Tokens.red : Tokens.line
    border.width: 1
    radius: Tokens.radius
    clip: true

    opacity: shown ? 1 : 0
    transform: Translate {
        x: root.shown ? 0 : Tokens.travel * 1.5
        Behavior on x {
            enabled: Tokens.motionEnabled
            NumberAnimation { duration: root.shown ? Tokens.base : Tokens.exitMs(Tokens.base); easing.type: Easing.BezierSpline; easing.bezierCurve: root.shown ? Tokens.curveOut : Tokens.curveIn }
        }
    }
    Behavior on opacity {
        NumberAnimation { duration: Tokens.motionEnabled ? (root.shown ? Tokens.fast : Tokens.exitMs(Tokens.fast)) : 0; easing.type: Easing.BezierSpline; easing.bezierCurve: root.shown ? Tokens.curveOut : Tokens.curveIn }
    }

    // a faixa é uma pílula recuada, não uma borda cortada pela quina
    Rectangle {
        x: Tokens.gapIn
        y: Tokens.gapOut
        width: 3
        height: parent.height - Tokens.gapOut * 2
        radius: 2
        color: root.urgent ? Tokens.red : Tokens.accent
    }

    Column {
        id: col
        anchors { left: parent.left; right: parent.right; verticalCenter: parent.verticalCenter; leftMargin: Tokens.gapOut + 6; rightMargin: Tokens.gapOut }
        spacing: 2
        Mono { text: root.app; small: true; color: Tokens.mute; width: parent.width; elide: Text.ElideRight; visible: text !== "" }
        Mono { text: root.summary; font.bold: true; width: parent.width; wrapMode: Text.Wrap; maximumLineCount: 2; elide: Text.ElideRight }
        Mono { text: root.body; color: Tokens.dim; width: parent.width; wrapMode: Text.Wrap; maximumLineCount: 4; elide: Text.ElideRight; visible: text !== "" }
        Row {
            visible: root.actions.length > 0
            spacing: Tokens.gapOut
            topPadding: Tokens.gapIn
            Repeater {
                model: root.actions
                delegate: Segment {
                    required property var modelData
                    text: modelData.text
                    color: Tokens.accent
                    implicitHeight: Tokens.text + Tokens.gapIn
                    onClicked: root.actionInvoked(modelData.id)
                }
            }
        }
    }

    MouseArea {
        anchors.fill: parent
        z: -1
        onClicked: root.dismissed()
    }
}
