import QtQuick
import qs.core

// A marca: um "c" de traço uniforme e o cursor de bloco dentro da boca.
// Geometria igual a brand/mark.svg (grade de 64): corpo = quadrado menos a boca x 20..64, y 20..44;
// cursor 12x24 em x=34. Os braços avançam meia unidade sobre a coluna para não deixar fresta.
Item {
    id: root
    property int size: 96
    property color body: Tokens.fg
    property color cursor: Tokens.accent
    property bool blink: false
    implicitWidth: size
    implicitHeight: size

    readonly property real u: size / 64

    Rectangle { x: 0; y: 0; width: 20 * root.u; height: root.size; color: root.body }
    Rectangle { x: 19.5 * root.u; y: 0; width: 44.5 * root.u; height: 20 * root.u; color: root.body }
    Rectangle { x: 19.5 * root.u; y: 44 * root.u; width: 44.5 * root.u; height: 20 * root.u; color: root.body }
    Rectangle {
        x: 34 * root.u; y: 20 * root.u; width: 12 * root.u; height: 24 * root.u
        color: root.cursor
        SequentialAnimation on opacity {
            running: root.blink && Tokens.motionEnabled
            loops: Animation.Infinite
            PauseAnimation { duration: 530 }
            NumberAnimation { to: 0; duration: 1 }
            PauseAnimation { duration: 530 }
            NumberAnimation { to: 1; duration: 1 }
        }
    }
}
