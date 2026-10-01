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
