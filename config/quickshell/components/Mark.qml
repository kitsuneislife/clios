import QtQuick
import QtQuick.Shapes
import qs.core

// A marca: um "c" de traço uniforme, com os cantos levemente arredondados, e o cursor de bloco
// dentro da boca. O caminho é o mesmo de brand/mark.svg (grade de 64); um teste confere os dois.
Item {
    id: root
    property int size: 96
    property color body: Tokens.fg
    property color cursor: Tokens.accent
    property bool blink: false
    implicitWidth: size
    implicitHeight: size

    readonly property real u: size / 64
    // MARK_PATH (gerado por brand/build.py --path)
    readonly property string markPath: "M0 6A6 6 0 0 1 6 0L58 0A6 6 0 0 1 64 6L64 17A3 3 0 0 1 61 20L22 20A2 2 0 0 0 20 22L20 42A2 2 0 0 0 22 44L61 44A3 3 0 0 1 64 47L64 58A6 6 0 0 1 58 64L6 64A6 6 0 0 1 0 58Z"

    Shape {
        width: 64
        height: 64
        scale: root.u
        transformOrigin: Item.TopLeft
        ShapePath {
            fillColor: root.body
            strokeColor: "transparent"
            PathSvg { path: root.markPath }
        }
    }

    Rectangle {
        x: 34 * root.u; y: 20 * root.u; width: 12 * root.u; height: 24 * root.u
        radius: 2 * root.u
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
