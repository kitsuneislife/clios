import QtQuick
import Quickshell
import Quickshell.Io
import Quickshell.Wayland
import Quickshell.Services.Pipewire
import qs.core
import qs.components

// Indicador de volume e brilho. O volume aparece sozinho, porque a shell observa o PipeWire
// (qualquer fonte de mudança, não só as teclas). O brilho é avisado por `qs ipc call osd brightness`.
Scope {
    id: root
    property string label: "vol"
    property real value: 0
    property bool muted: false
    property bool shown: false
    property bool mapped: false
    // Nos primeiros segundos o PipeWire ainda está acordando; sem isso o OSD pisca ao iniciar.
    property bool ready: false

    Timer { interval: 2000; running: true; onTriggered: root.ready = true }
    Timer { id: hideTimer; interval: 1400; onTriggered: root.shown = false }
    // Mantém a janela mapeada enquanto o fade-out termina.
    Timer { id: unmapTimer; interval: Tokens.base + 60; onTriggered: root.mapped = false }

    function show(label, value, muted) {
        if (!ready) return
        root.label = label
        root.value = Math.max(0, Math.min(1, value))
        root.muted = muted
        root.mapped = true
        root.shown = true
        unmapTimer.stop()
        hideTimer.restart()
    }

    onShownChanged: if (!shown) unmapTimer.restart()

    PwObjectTracker { objects: [Pipewire.defaultAudioSink] }
    Connections {
        target: Pipewire.defaultAudioSink ? Pipewire.defaultAudioSink.audio : null
        function onVolumesChanged() { root.show("vol", target.volume, target.muted) }
        function onMutedChanged() { root.show("vol", target.volume, target.muted) }
    }

    IpcHandler {
        target: "osd"
        function brightness(): void { brightnessProc.running = true }
    }
    Process {
        id: brightnessProc
        command: ["brightnessctl", "-m"] // dispositivo,classe,atual,percentual,máximo
        stdout: StdioCollector {
            onStreamFinished: {
                const pct = parseInt(text.trim().split(",")[3])
                if (!isNaN(pct)) root.show("brilho", pct / 100, false)
            }
        }
    }

    LazyLoader {
        active: root.mapped

        PanelWindow {
            screen: Quickshell.screens[0]
            anchors.bottom: true
            margins.bottom: 72
            implicitWidth: view.implicitWidth
            implicitHeight: view.implicitHeight
            exclusionMode: ExclusionMode.Ignore
            WlrLayershell.layer: WlrLayer.Overlay
            WlrLayershell.namespace: "clios-osd"
            color: "transparent"
            mask: Region {} // não captura o mouse: dá para clicar através

            OsdView {
                id: view
                shown: root.shown
                label: root.label
                value: root.value
                muted: root.muted
            }
        }
    }
}
