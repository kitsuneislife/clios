pragma Singleton

import QtQuick

// Os tokens de design, no QML. Os valores padrão são os de tokens/tokens.toml (escuro, ember);
// `apply()` os troca em tempo real quando `clios theme apply` regrava theme.json.
// Quem muda de cor anima de cor: todo `color` consumidor deve ter `Behavior on color`.
QtObject {
    id: root

    // ── cor ────────────────────────────────────────────────────────────────
    property color bg: "#000000"
    property color surface: "#0A0A0A"
    property color raised: "#151515"
    property color line: "#2A2A2A"
    property color mute: "#6B6B6B"
    property color dim: "#A8A8A8"
    property color fg: "#F5F5F5"
    property color accent: "#FF5A1F"
    property color accentDim: "#8F3412"
    property color accentSoft: "#2E1006"
    // Cor de texto legível sobre `accent`. (Não pode se chamar `onAccent`: o QML leria como handler de sinal.)
    property color accentInk: "#000000"
    property color red: "#FF6166"
    property color green: "#4FCB8B"
    property color yellow: "#E8BE4F"

    // ── tipografia e grade ─────────────────────────────────────────────────
    property string mono: "GeistMono Nerd Font"
    property int text: 13
    property int textSmall: 11
    property int gapIn: 6
    property int gapOut: 12
    property int border: 2
    property int radius: 8
    property int radiusSmall: 4
    property int barHeight: 28

    // ── movimento (ms) ─────────────────────────────────────────────────────
    property bool motionEnabled: true
    property bool motionSpatial: true
    property int instant: 90
    property int fast: 160
    property int base: 240
    property int slow: 360
    // Pontos de controle do bezier cúbico no formato do QML: [x1, y1, x2, y2, 1, 1].
    property var curveOut: [0.16, 1.0, 0.30, 1.0, 1.0, 1.0]
    property var curveIn: [0.70, 0.0, 0.84, 0.0, 1.0, 1.0]
    property var curveInOut: [0.65, 0.0, 0.35, 1.0, 1.0, 1.0]

    // A saída dura ~60% da entrada (princípio 1 do manifesto de movimento).
    function exitMs(ms) { return Math.round(ms * 0.6) }

    // Quanto desloca ao entrar (px); zero sem movimento espacial.
    readonly property int travel: motionSpatial ? 16 : 0

    property string mode: "dark"
    property string accentName: "ember"

    function curve(points) { return points.length === 4 ? points.concat([1.0, 1.0]) : points }

    // Recebe o objeto de theme.json. Chaves ausentes mantêm o valor atual: um theme.json
    // de uma versão antiga nunca deixa a shell sem cor.
    function apply(t) {
        if (!t) return
        const c = t.color || {}
        for (const k of ["bg", "surface", "raised", "line", "mute", "dim", "fg", "accent",
                         "accentDim", "accentSoft", "red", "green", "yellow"]) {
            if (c[k] !== undefined) root[k] = c[k]
        }
        if (c.onAccent !== undefined) accentInk = c.onAccent
        if (t.font && t.font.mono) mono = t.font.mono
        const u = t.ui || {}
        for (const k of ["text", "textSmall", "gapIn", "gapOut", "border", "radius", "radiusSmall", "barHeight"]) {
            if (u[k] !== undefined) root[k] = u[k]
        }
        const m = t.motion || {}
        if (m.enabled !== undefined) motionEnabled = m.enabled
        if (m.spatial !== undefined) motionSpatial = m.spatial
        for (const k of ["instant", "fast", "base", "slow"]) {
            if (m[k] !== undefined) root[k] = m[k]
        }
        if (m.curve) {
            if (m.curve.out) curveOut = curve(m.curve.out)
            if (m.curve.in) curveIn = curve(m.curve["in"])
            if (m.curve.inout) curveInOut = curve(m.curve.inout)
        }
        if (t.mode) mode = t.mode
        if (t.accent) accentName = t.accent
    }
}
