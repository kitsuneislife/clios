pragma Singleton

import QtQuick
import Quickshell
import Quickshell.Io

// Conexão de rede. `clios status net` (Rust) lê /sys e /proc; a barra só pergunta a cada 5 s.
// Não usa o módulo de rede do Quickshell porque ele exige NetworkManager, e aqui o wi-fi é do iwd.
Singleton {
    property string kind: "none"
    property string label: ""

    Process {
        id: poll
        command: ["clios", "status", "net"]
        stdout: StdioCollector {
            onStreamFinished: {
                try {
                    const j = JSON.parse(text)
                    kind = j.kind
                    label = j.label
                } catch (e) {
                    // mantém o último estado conhecido
                }
            }
        }
    }

    Timer {
        interval: 5000
        running: true
        repeat: true
        triggeredOnStart: true
        onTriggered: poll.running = true
    }
}
