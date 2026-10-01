import QtQuick
import qs.core

// Um trecho de texto clicável da barra: o rótulo esmaece até a cor de texto no hover.
Item {
    id: root
    property alias text: label.text
    property color color: Tokens.dim
    property bool urgent: false
    property bool hoverable: true
    signal clicked

    implicitWidth: label.implicitWidth + Tokens.gapOut
    implicitHeight: parent ? parent.height : Tokens.barHeight

    // no hover, uma pílula suave por trás do texto
    Rectangle {
        anchors { fill: parent; topMargin: 4; bottomMargin: 4; leftMargin: 2; rightMargin: 2 }
        radius: Tokens.radiusSmall
        color: Tokens.raised
        opacity: area.containsMouse && root.hoverable ? 1 : 0
        Behavior on opacity { NumberAnimation { duration: Tokens.motionEnabled ? Tokens.instant : 0 } }
    }

    Mono {
        id: label
        anchors.centerIn: parent
        color: root.urgent ? Tokens.red : (area.containsMouse && root.hoverable ? Tokens.fg : root.color)
    }

    MouseArea {
        id: area
        anchors.fill: parent
        hoverEnabled: true
        cursorShape: root.hoverable ? Qt.PointingHandCursor : Qt.ArrowCursor
        onClicked: root.clicked()
    }
}
