pragma Singleton
import QtQuick

// Colours derived from the platform palette so light and dark themes work.
QtObject {
    readonly property SystemPalette pal: SystemPalette { colorGroup: SystemPalette.Active }
    readonly property SystemPalette disabledPal: SystemPalette { colorGroup: SystemPalette.Disabled }
    readonly property bool dark: pal.window.hslLightness < 0.5

    readonly property color text: pal.text
    readonly property color windowText: pal.windowText
    readonly property color base: pal.base
    readonly property color alternateBase: pal.alternateBase
    readonly property color window: pal.window
    readonly property color mid: pal.mid
    readonly property color highlight: pal.highlight
    readonly property color highlightedText: pal.highlightedText
    readonly property color disabledText: disabledPal.text
    readonly property color muted: Qt.rgba(text.r, text.g, text.b, 0.55)
    readonly property color grid: Qt.rgba(text.r, text.g, text.b, dark ? 0.16 : 0.10)
    readonly property color frame: Qt.rgba(text.r, text.g, text.b, 0.35)
    readonly property color selection: Qt.rgba(highlight.r, highlight.g, highlight.b, 0.28)

    readonly property color warn: dark ? "#f0a040" : "#b05a00"
    readonly property color error: dark ? "#ff7b72" : "#c62828"
    readonly property color good: dark ? "#7ccf8a" : "#2e7d32"

    // Column tints by role.
    readonly property color envTint: dark ? "#1d3a38" : "#e2f3f0"
    readonly property color noiseTint: dark ? "#3a2f1d" : "#fbf0df"
    readonly property color blockTint: dark ? "#2f2f2f" : "#efefef"
    readonly property color respTint: dark ? "#2b2944" : "#ebeafb"
    readonly property color needTint: dark ? "#3f3720" : "#fff4d6"
    readonly property color envAccent: dark ? "#55c2b5" : "#14897c"
    readonly property color noiseAccent: dark ? "#e0a050" : "#b8741a"
    readonly property color respAccent: dark ? "#9d97f0" : "#5b52c9"

    readonly property var seriesLight: ["#2f6fb2", "#d0782a", "#3c9a5f", "#8e4fb5", "#c0392b", "#6b6b6b", "#1b9aaa", "#b5873a"]
    readonly property var seriesDark: ["#6aa9e9", "#f0a35c", "#6cc48a", "#c08ae0", "#ef7b73", "#b9b9b9", "#5fd0dc", "#e0be6e"]

    function seriesColor(i) {
        if (i === -1) return muted
        if (i === -2) return Qt.rgba(text.r, text.g, text.b, 0.75)
        if (i === -3) return error
        const s = dark ? seriesDark : seriesLight
        return s[((i % s.length) + s.length) % s.length]
    }

    // Perceptual sequential colour ramp (viridis-like), t in [0, 1].
    readonly property var ramp: [[68, 1, 84], [65, 68, 135], [42, 120, 142], [34, 168, 132], [122, 209, 81], [253, 231, 37]]
    function heat(t) {
        if (!isFinite(t)) return "transparent"
        t = Math.max(0, Math.min(1, t)) * (ramp.length - 1)
        const i = Math.min(ramp.length - 2, Math.floor(t))
        const u = t - i
        const a = ramp[i], b = ramp[i + 1]
        return Qt.rgba((a[0] + u * (b[0] - a[0])) / 255, (a[1] + u * (b[1] - a[1])) / 255, (a[2] + u * (b[2] - a[2])) / 255, 1)
    }

    function tintForKind(kind) {
        switch (kind) {
        case "env": return envTint
        case "noise": return noiseTint
        case "block": return blockTint
        case "response": return respTint
        default: return "transparent"
        }
    }

    function accentForKind(kind) {
        switch (kind) {
        case "env": return envAccent
        case "noise": return noiseAccent
        case "response": return respAccent
        default: return "transparent"
        }
    }
}
