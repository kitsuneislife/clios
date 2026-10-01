import QtQuick
import qs.core

// O único componente de texto do desktop: sempre a mesma fonte, sempre das cores dos tokens.
Text {
    property bool small: false
    color: Tokens.fg
    font.family: Tokens.mono
    font.pixelSize: small ? Tokens.textSmall : Tokens.text
    renderType: Text.NativeRendering
    verticalAlignment: Text.AlignVCenter
    Behavior on color { ColorAnimation { duration: Tokens.motionEnabled ? Tokens.fast : 0; easing.type: Easing.BezierSpline; easing.bezierCurve: Tokens.curveOut } }
}
