pragma Singleton

import QtQuick
import Quickshell
import Quickshell.Io

// O estado do sistema que a barra mostra: rede e os liga-desliga (café, noturno, não perturbe, gravação, foco).
// `clios status all` (Rust) lê /sys, /proc e os arquivos de estado; a barra só pergunta a cada 2 s.
// Não usa o módulo de rede do Quickshell porque ele exige NetworkManager, e aqui o wi-fi é do iwd.
Singleton {
    property string kind: "none"
    property string label: ""
    property bool caffeine: false
    property bool night: false
    property bool dnd: false
    property bool rec: false
    property int focusLeft: 0           // segundos que faltam do bloco de foco; 0 sem bloco
    property bool reboot: false         // o kernel foi atualizado e falta reiniciar

    Process {
        id: poll
        command: ["clios", "status", "all"]
        stdout: StdioCollector {
            onStreamFinished: {
                try {
                    const j = JSON.parse(text)
                    kind = j.net.kind
                    label = j.net.label
                    caffeine = j.caffeine
                    night = j.night
                    dnd = j.dnd
                    rec = j.rec
                    focusLeft = j.focus || 0
                    reboot = j.reboot === true
                } catch (e) {
                    // mantém o último estado conhecido
                }
            }
        }
    }

    Timer {
        interval: 2000
        running: true
        repeat: true
        triggeredOnStart: true
        onTriggered: poll.running = true
    }
}
