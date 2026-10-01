import QtQuick
import qs.core

// Papel de parede com troca em fade: a imagem nova carrega escondida, entra por cima na curva
// de chegada e vira a base. Sem imagem (ou enquanto carrega) mostra a cor de fundo.
Item {
    id: root

    property url source
    property bool topIsA: true
    property var pending: null

    readonly property Image front: topIsA ? a : b
    readonly property Image back: topIsA ? b : a
    readonly property bool ready: front.status === Image.Ready && front.opacity > 0.99

    function show(url) {
        if (!url || url.toString() === "") return
        if (fade.running) fade.complete()
        if (front.source.toString() === "") {
            pending = front
            front.source = url
        } else {
            pending = back
            back.opacity = 0
            back.z = 2
            front.z = 1
            back.source = url
        }
    }
    onSourceChanged: show(source)
    Component.onCompleted: show(source)

    function settle(img) {
        if (img !== pending) return
        fade.target = img
        fade.restart()
    }

    Rectangle { anchors.fill: parent; color: Tokens.bg }

    component Layer: Image {
        anchors.fill: parent
        fillMode: Image.PreserveAspectCrop
        asynchronous: true
        cache: false
        smooth: true
        opacity: 0
        onStatusChanged: if (status === Image.Ready) root.settle(this)
    }
    Layer { id: a }
    Layer { id: b }

    NumberAnimation {
        id: fade
        property: "opacity"
        to: 1
        duration: Tokens.motionEnabled ? Tokens.slow : 0
        easing.type: Easing.BezierSpline
        easing.bezierCurve: Tokens.curveOut
        onFinished: {
            if (target !== root.front) {
                const old = root.front
                root.topIsA = !root.topIsA
                old.source = ""
                old.opacity = 0
            }
        }
    }
}
