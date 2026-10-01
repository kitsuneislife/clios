import QtQuick
import Quickshell
import Quickshell.Services.UPower

// Avisos de bateria fraca: um aos 20% e outro, urgente, aos 10%. Cada um avisa uma vez só por descarga:
// ligar o carregador zera a conta. Em máquina sem bateria, não faz nada.
Scope {
    readonly property var dev: UPower.displayDevice
    readonly property bool present: dev !== null && dev.isPresent && dev.isLaptopBattery
    // UPower já devolveu 0..1 e 0..100 em versões diferentes; aceita os dois.
    readonly property real pct: present ? (dev.percentage > 1 ? dev.percentage : dev.percentage * 100) : 100
    readonly property bool discharging: present && dev.state === UPowerDeviceState.Discharging

    // 0: nenhum aviso ainda, 1: o dos 20%, 2: o dos 10%
    property int warned: 0

    onPctChanged: check()
    onDischargingChanged: check()

    function check() {
        if (!discharging) { warned = 0; return }
        const p = Math.round(pct)
        if (p <= 10 && warned < 2) {
            warned = 2
            Quickshell.execDetached(["notify-send", "-u", "critical", "-a", "bateria", "Bateria em " + p + "%", "Ligue o carregador."])
        } else if (p <= 20 && warned < 1) {
            warned = 1
            Quickshell.execDetached(["notify-send", "-a", "bateria", "Bateria em " + p + "%", "Falta pouco. O modo economia ajuda: clios power power-saver."])
        }
    }
}
