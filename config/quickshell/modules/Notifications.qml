pragma ComponentBehavior: Bound

import QtQuick
import Quickshell
import Quickshell.Io
import Quickshell.Services.Notifications
import Quickshell.Wayland
import qs.core
import qs.components

// O servidor de notificações do desktop. Cartões no canto superior direito, empilhados.
// Normal some em 7 s, baixa em 4 s, urgente fica até ser dispensada.
Scope {
    // Não perturbe: `clios dnd` grava 1 neste arquivo. As urgentes continuam passando.
    readonly property string stateHome: Quickshell.env("XDG_STATE_HOME") || (Quickshell.env("HOME") + "/.local/state")
    property bool dnd: false

    FileView {
        path: stateHome + "/clios/dnd"
        watchChanges: true
        printErrors: false
        onFileChanged: reload()
        onLoaded: dnd = text().trim() === "1"
        onLoadFailed: dnd = false
    }

    NotificationServer {
        id: server
        keepOnReload: true
        actionsSupported: true
        bodySupported: true
        imageSupported: false
        onNotification: n => {
            const silenced = dnd && n.urgency !== NotificationUrgency.Critical
            // Tudo vai para o histórico (`clios notifs`, super + shift + n), inclusive o que o não perturbe silencia.
            // O formato `--campo=valor` impede que um texto começando com "-" seja lido como opção.
            const urgency = n.urgency === NotificationUrgency.Critical ? "critical"
                : n.urgency === NotificationUrgency.Low ? "low" : "normal"
            Quickshell.execDetached(["clios", "notifs", "add",
                "--app=" + n.appName, "--summary=" + n.summary, "--body=" + n.body, "--urgency=" + urgency]
                .concat(silenced ? ["--silenced"] : []))
            if (silenced) return
            n.tracked = true
        }
    }

    PanelWindow {
        id: win
        screen: Quickshell.screens[0]
        visible: server.trackedNotifications.values.length > 0
        anchors { top: true; right: true }
        margins { top: Tokens.barHeight + Tokens.gapIn + Tokens.gapOut; right: Tokens.gapOut }
        implicitWidth: 360
        implicitHeight: stack.implicitHeight
        exclusionMode: ExclusionMode.Ignore
        WlrLayershell.layer: WlrLayer.Overlay
        WlrLayershell.namespace: "clios-notify"
        color: "transparent"

        Column {
            id: stack
            width: parent.width
            spacing: Tokens.gapIn

            Repeater {
                model: server.trackedNotifications

                delegate: ToastView {
                    id: toast
                    required property var modelData
                    width: stack.width
                    app: modelData.appName
                    summary: modelData.summary
                    body: modelData.body
                    urgent: modelData.urgency === NotificationUrgency.Critical
                    actions: modelData.actions.map(a => ({ id: a.identifier, text: a.text }))

                    // `shown` anima a entrada; para sair, anima primeiro e só depois remove do modelo.
                    Component.onCompleted: shown = true
                    function leave(fn) {
                        shown = false
                        exitTimer.callback = fn
                        exitTimer.restart()
                    }
                    Timer {
                        id: exitTimer
                        property var callback
                        interval: Tokens.exitMs(Tokens.base) + 40
                        onTriggered: if (callback) callback()
                    }

                    Timer {
                        // urgente (2) não expira sozinha
                        running: toast.modelData.urgency !== NotificationUrgency.Critical
                        interval: toast.modelData.expireTimeout > 0
                            ? toast.modelData.expireTimeout * 1000
                            : (toast.modelData.urgency === NotificationUrgency.Low ? 4000 : 7000)
                        onTriggered: toast.leave(() => toast.modelData.expire())
                    }

                    onDismissed: leave(() => modelData.dismiss())
                    onActionInvoked: id => {
                        for (const a of modelData.actions) if (a.identifier === id) a.invoke()
                        leave(() => modelData.dismiss())
                    }
                }
            }
        }
    }
}
