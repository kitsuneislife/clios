import QtQuick
import Quickshell
import Quickshell.Hyprland
import Quickshell.Wayland
import Quickshell.Services.Pipewire
import Quickshell.Services.UPower
import qs.core
import qs.components

// A barra: uma por monitor, no topo, 28px. Dados do Hyprland, PipeWire e UPower;
// cliques abrem a TUI correspondente numa janela flutuante.
Scope {
    SystemClock { id: clock; precision: SystemClock.Minutes }
    PwObjectTracker { objects: [Pipewire.defaultAudioSink] }

    function open(id) { Quickshell.execDetached(["clios", "open", id]) }

    Variants {
        model: Quickshell.screens

        PanelWindow {
            id: win
            required property var modelData
            screen: modelData

            anchors { top: true; left: true; right: true }
            // a barra flutua: margem em volta, para o canto arredondado aparecer
            margins { top: Tokens.gapIn; left: Tokens.gapOut; right: Tokens.gapOut }
            implicitHeight: Tokens.barHeight
            exclusionMode: ExclusionMode.Auto
            WlrLayershell.namespace: "clios-bar"
            color: "transparent"

            readonly property var monitor: Hyprland.monitorFor(modelData)
            readonly property var sink: Pipewire.defaultAudioSink
            readonly property var battery: UPower.displayDevice

            // `Hyprland.dispatch` fala Lua nas versões novas do Hyprland e o dialeto antigo nas velhas.
            function goTo(id) {
                Hyprland.dispatch(Hyprland.usingLua
                    ? "hl.dsp.focus({ workspace = " + id + " })"
                    : "workspace " + id)
            }

            readonly property var workspaceList: {
                const out = []
                for (const w of Hyprland.workspaces.values) {
                    if (w.id < 1) continue // workspaces especiais (scratchpad) têm id negativo
                    out.push({
                        id: w.id,
                        active: monitor && monitor.activeWorkspace ? monitor.activeWorkspace.id === w.id : false,
                        occupied: w.toplevels.values.length > 0,
                        urgent: w.urgent
                    })
                }
                out.sort((a, b) => a.id - b.id)
                return out
            }

            // UPower já devolveu 0..1 e 0..100 em versões diferentes; aceita os dois.
            function fraction(p) { return p > 1 ? p / 100 : p }

            BarView {
                anchors.fill: parent
                workspaces: win.workspaceList
                title: Hyprland.activeToplevel ? Hyprland.activeToplevel.title : ""
                audioKnown: win.sink !== null
                volume: win.sink && win.sink.audio ? win.sink.audio.volume : 0
                muted: win.sink && win.sink.audio ? win.sink.audio.muted : false
                netKind: Status.kind
                netLabel: Status.label
                rec: Status.rec
                caffeine: Status.caffeine
                night: Status.night
                dnd: Status.dnd
                battery: win.battery && win.battery.isPresent && win.battery.isLaptopBattery ? win.fraction(win.battery.percentage) : -1
                charging: win.battery ? win.battery.state === UPowerDeviceState.Charging : false
                clock: Qt.formatDateTime(clock.date, "ddd d  HH:mm")

                onWorkspaceActivated: id => win.goTo(id)
                onAudioClicked: open("audio")
                onNetClicked: open("network")
                onBatteryClicked: open("monitor")
                onClockClicked: open("monitor")
                // clicar num indicador desliga o que ele mostra
                onIndicatorClicked: name => Quickshell.execDetached(name === "rec" ? ["clios", "rec", "stop"] : ["clios", name, "off"])
            }
        }
    }
}
