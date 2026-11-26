import QtQuick
import QtQuick.Controls
import Sdoe

// Draws an engine plot description (axes, series, heat maps, 3D polygons).
Item {
    id: root
    property var plot: null
    property bool compact: false
    property real azimuth: 35
    property real elevation: 30
    signal clicked(real x, real y)
    signal pointClicked(int id)
    signal rotated(real azimuth, real elevation)

    implicitWidth: 360
    implicitHeight: 280

    onPlotChanged: canvas.requestPaint()
    Connections {
        target: Theme
        function onDarkChanged() { canvas.requestPaint() }
    }

    // Layout of the last paint for hit testing.
    property var _geom: null
    property var _points: []

    readonly property string noteText: plot && plot.note && !/^\d+$/.test(plot.note) ? plot.note : ""

    Canvas {
        id: canvas
        anchors.fill: parent
        anchors.bottomMargin: noteLabel.visible ? noteLabel.height + 2 : 0
        renderStrategy: Canvas.Cooperative
        onPaint: root.paint(getContext("2d"))
        onWidthChanged: requestPaint()
        onHeightChanged: requestPaint()
    }

    Label {
        id: noteLabel
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.bottom: parent.bottom
        anchors.leftMargin: 8
        visible: text.length > 0
        text: root.noteText
        wrapMode: Text.WordWrap
        color: Theme.muted
        font.pointSize: Qt.application.font.pointSize * 0.92
    }

    function font(px, bold) {
        return (bold ? "bold " : "") + px + "px '" + Qt.application.font.family + "'"
    }

    function paint(ctx) {
        const w = canvas.width, h = canvas.height
        ctx.reset()
        ctx.clearRect(0, 0, w, h)
        root._points = []
        const p = root.plot
        if (!p || w < 40 || h < 40) return
        const fs = root.compact ? 10 : 11
        const txt = Theme.text.toString()
        const is3d = p.kind === "3d"
        const isMatrix = p.kind === "matrix"
        const hasScale = !!p.value_range || (p.heatmap && !isMatrix)

        // Margins.
        ctx.font = root.font(fs, false)
        let ml = 8, mr = 10, mt = p.title ? 22 : 8, mb = 8
        let xRot = 0
        if (!is3d) {
            let maxY = 0
            for (const t of p.y.ticks) maxY = Math.max(maxY, ctx.measureText(t.label).width)
            ml = maxY + 12 + (p.y_label ? 18 : 0)
            let maxX = 0
            for (const t of p.x.ticks) maxX = Math.max(maxX, ctx.measureText(t.label).width)
            const spacing = p.x.ticks.length > 1 ? (w - ml - mr) / p.x.ticks.length : w
            if (maxX > spacing * 0.95) xRot = 1
            mb = (xRot ? Math.min(maxX * 0.75, h * 0.35) + 10 : 20) + (p.x_label ? 18 : 0)
        }
        if (hasScale) mr += 54
        const pl = ml, pt = mt, pw = Math.max(10, w - ml - mr), ph = Math.max(10, h - mt - mb)
        const x0 = p.x.min, x1 = p.x.max, y0 = p.y.min, y1 = p.y.max
        const sx = x => pl + (x - x0) / (x1 - x0) * pw
        const sy = y => pt + ph - (y - y0) / (y1 - y0) * ph
        root._geom = { pl: pl, pt: pt, pw: pw, ph: ph, x0: x0, x1: x1, y0: y0, y1: y1 }

        if (p.title) {
            ctx.fillStyle = txt
            ctx.font = root.font(fs + 1, true)
            ctx.textAlign = "center"
            ctx.textBaseline = "alphabetic"
            ctx.fillText(p.title, w / 2, 15)
            ctx.font = root.font(fs, false)
        }

        // Grid.
        if (!is3d && !isMatrix) {
            ctx.strokeStyle = Theme.grid.toString()
            ctx.lineWidth = 1
            ctx.beginPath()
            for (const t of p.x.ticks) { const x = Math.round(sx(t.v)) + 0.5; ctx.moveTo(x, pt); ctx.lineTo(x, pt + ph) }
            for (const t of p.y.ticks) { const y = Math.round(sy(t.v)) + 0.5; ctx.moveTo(pl, y); ctx.lineTo(pl + pw, y) }
            ctx.stroke()
        }

        // Heat map.
        if (p.heatmap) {
            const hm = p.heatmap
            const span = (hm.vmax - hm.vmin) || 1
            ctx.save()
            ctx.beginPath()
            ctx.rect(pl, pt, pw, ph)
            ctx.clip()
            if (isMatrix) {
                for (let j = 0; j < hm.ny; j++) for (let i = 0; i < hm.nx; i++) {
                    const v = hm.values[j * hm.nx + i]
                    ctx.fillStyle = Theme.heat((v - hm.vmin) / span).toString()
                    const xa = sx(i - 0.5), xb = sx(i + 0.5), ya = sy(j - 0.5), yb = sy(j + 0.5)
                    ctx.fillRect(Math.min(xa, xb), Math.min(ya, yb), Math.abs(xb - xa) + 0.5, Math.abs(yb - ya) + 0.5)
                }
            } else {
                const dx = (hm.x1 - hm.x0) / Math.max(1, hm.nx - 1), dy = (hm.y1 - hm.y0) / Math.max(1, hm.ny - 1)
                for (let j = 0; j < hm.ny; j++) for (let i = 0; i < hm.nx; i++) {
                    const v = hm.values[j * hm.nx + i]
                    if (v === null || !isFinite(v)) continue
                    ctx.fillStyle = Theme.heat((v - hm.vmin) / span).toString()
                    const cx = hm.x0 + i * dx, cy = hm.y0 + j * dy
                    const xa = sx(cx - dx / 2), xb = sx(cx + dx / 2), ya = sy(cy + dy / 2), yb = sy(cy - dy / 2)
                    ctx.fillRect(xa, ya, xb - xa + 0.6, yb - ya + 0.6)
                }
            }
            ctx.restore()
        }

        // Clip series to the plot area.
        ctx.save()
        if (!is3d) {
            ctx.beginPath()
            ctx.rect(pl - 1, pt - 1, pw + 2, ph + 2)
            ctx.clip()
        }
        const vr = p.value_range
        for (const s of p.series) root.drawSeries(ctx, s, sx, sy, pl, pt, pw, ph, vr, fs)
        ctx.restore()

        // Axes.
        if (!is3d) {
            ctx.strokeStyle = Theme.frame.toString()
            ctx.lineWidth = 1
            ctx.strokeRect(pl + 0.5, pt + 0.5, pw, ph)
            ctx.fillStyle = txt
            ctx.font = root.font(fs, false)
            ctx.textBaseline = "top"
            for (const t of p.x.ticks) {
                const x = sx(t.v)
                if (x < pl - 1 || x > pl + pw + 1) continue
                if (xRot) {
                    ctx.save()
                    ctx.translate(x + 3, pt + ph + 5)
                    ctx.rotate(-Math.PI / 4)
                    ctx.textAlign = "right"
                    ctx.textBaseline = "middle"
                    ctx.fillText(t.label, 0, 0)
                    ctx.restore()
                } else {
                    ctx.textAlign = "center"
                    ctx.fillText(t.label, x, pt + ph + 4)
                }
            }
            ctx.textAlign = "right"
            ctx.textBaseline = "middle"
            for (const t of p.y.ticks) {
                const y = sy(t.v)
                if (y < pt - 1 || y > pt + ph + 1) continue
                ctx.fillText(t.label, pl - 5, y)
            }
            if (p.x_label) {
                ctx.textAlign = "center"
                ctx.textBaseline = "bottom"
                ctx.fillText(p.x_label, pl + pw / 2, h - 2)
            }
            if (p.y_label) {
                ctx.save()
                ctx.translate(11, pt + ph / 2)
                ctx.rotate(-Math.PI / 2)
                ctx.textAlign = "center"
                ctx.textBaseline = "middle"
                ctx.fillText(p.y_label, 0, 0)
                ctx.restore()
            }
        }

        // Colour scale.
        if (hasScale) {
            const lo = vr ? vr[0] : p.heatmap.vmin, hi = vr ? vr[1] : p.heatmap.vmax
            const bx = w - mr + 10, by = pt + 4, bh = ph - 8
            for (let i = 0; i < bh; i++) {
                ctx.fillStyle = Theme.heat(1 - i / bh).toString()
                ctx.fillRect(bx, by + i, 12, 1.5)
            }
            ctx.fillStyle = txt
            ctx.textAlign = "left"
            ctx.textBaseline = "middle"
            ctx.fillText(root.fmt(hi), bx + 15, by + 4)
            ctx.fillText(root.fmt(lo), bx + 15, by + bh - 4)
        }

        // Legend.
        if (p.legend) {
            const items = p.series.filter(s => s.name && s.kind !== "labels" && s.kind !== "segments")
            if (items.length > 0) {
                ctx.font = root.font(fs, false)
                let lw = 0
                for (const s of items) lw = Math.max(lw, ctx.measureText(s.name).width)
                const bottom = p.series.some(s => s.kind === "hbars")
                const lh = 16
                const lx = pl + pw - lw - 34, ly = bottom ? pt + ph - items.length * lh - 6 : pt + 6
                ctx.fillStyle = Theme.base.toString()
                ctx.globalAlpha = 0.85
                ctx.fillRect(lx - 4, ly - 4, lw + 34, items.length * lh + 6)
                ctx.globalAlpha = 1
                items.forEach((s, i) => {
                    const y = ly + i * lh + 6
                    ctx.strokeStyle = ctx.fillStyle = Theme.seriesColor(s.color).toString()
                    ctx.lineWidth = 2
                    ctx.setLineDash(s.dashed ? [4, 3] : [])
                    if (s.kind === "points") { ctx.beginPath(); ctx.arc(lx + 9, y, 3.5, 0, 2 * Math.PI); ctx.fill() }
                    else { ctx.beginPath(); ctx.moveTo(lx, y); ctx.lineTo(lx + 18, y); ctx.stroke() }
                    ctx.setLineDash([])
                    ctx.fillStyle = txt
                    ctx.textAlign = "left"
                    ctx.textBaseline = "middle"
                    ctx.fillText(s.name, lx + 24, y)
                })
            }
        }
    }

    function fmt(v) {
        if (v === null || v === undefined || !isFinite(v)) return ""
        const a = Math.abs(v)
        if (a !== 0 && (a >= 1e5 || a < 1e-3)) return v.toExponential(2)
        return Number(v.toPrecision(4)).toString()
    }

    function drawSeries(ctx, s, sx, sy, pl, pt, pw, ph, vr, fs) {
        const col = Theme.seriesColor(s.color).toString()
        ctx.strokeStyle = col
        ctx.fillStyle = col
        ctx.lineWidth = 1.8
        ctx.setLineDash(s.dashed ? [5, 4] : [])
        const n = s.x.length
        switch (s.kind) {
        case "line":
        case "linepoints": {
            ctx.beginPath()
            let started = false
            for (let i = 0; i < n; i++) {
                if (s.y[i] === null || !isFinite(s.y[i])) { started = false; continue }
                const X = sx(s.x[i]), Y = sy(s.y[i])
                if (!started) { ctx.moveTo(X, Y); started = true } else ctx.lineTo(X, Y)
            }
            ctx.stroke()
            if (s.kind === "linepoints") {
                ctx.setLineDash([])
                for (let i = 0; i < n; i++) {
                    if (!isFinite(s.y[i])) continue
                    ctx.beginPath(); ctx.arc(sx(s.x[i]), sy(s.y[i]), 3.5, 0, 2 * Math.PI); ctx.fill()
                }
            }
            break
        }
        case "points": {
            ctx.setLineDash([])
            for (let i = 0; i < n; i++) {
                if (s.x[i] === null || s.y[i] === null || !isFinite(s.x[i]) || !isFinite(s.y[i])) continue
                const X = sx(s.x[i]), Y = sy(s.y[i])
                ctx.beginPath()
                ctx.arc(X, Y, root.compact ? 3 : 3.8, 0, 2 * Math.PI)
                ctx.fill()
                if (s.color === -1) { ctx.strokeStyle = Theme.base.toString(); ctx.lineWidth = 1; ctx.stroke(); ctx.strokeStyle = col }
                root._points.push({ x: X, y: Y, id: s.ids && s.ids.length ? s.ids[i] : -1, label: s.labels && s.labels.length ? s.labels[i] : "", dx: s.x[i], dy: s.y[i] })
            }
            break
        }
        case "band": {
            ctx.globalAlpha = 0.18
            ctx.beginPath()
            for (let i = 0; i < n; i++) { const X = sx(s.x[i]), Y = sy(s.y[i]); if (i === 0) ctx.moveTo(X, Y); else ctx.lineTo(X, Y) }
            for (let i = n - 1; i >= 0; i--) ctx.lineTo(sx(s.x[i]), sy(s.y2[i]))
            ctx.closePath()
            ctx.fill()
            ctx.globalAlpha = 1
            break
        }
        case "errorbars": {
            ctx.setLineDash([])
            for (let i = 0; i < n; i++) {
                const X = sx(s.x[i])
                ctx.beginPath()
                ctx.moveTo(X, sy(s.y[i])); ctx.lineTo(X, sy(s.y2[i]))
                ctx.moveTo(X - 5, sy(s.y[i])); ctx.lineTo(X + 5, sy(s.y[i]))
                ctx.moveTo(X - 5, sy(s.y2[i])); ctx.lineTo(X + 5, sy(s.y2[i]))
                ctx.stroke()
            }
            break
        }
        case "bars": {
            let step = pw / Math.max(1, n)
            if (n > 1) step = Math.min(step, Math.abs(sx(s.x[1]) - sx(s.x[0])))
            const bw = Math.max(2, Math.min(28, step * 0.7))
            const base = sy(Math.max(root.plot.y.min, Math.min(0, root.plot.y.max)))
            for (let i = 0; i < n; i++) {
                const X = sx(s.x[i]), Y = sy(s.y[i])
                ctx.fillRect(X - bw / 2, Math.min(Y, base), bw, Math.abs(base - Y))
            }
            break
        }
        case "hbars": {
            const bh = Math.max(3, Math.min(22, ph / Math.max(1, n) * 0.7))
            const base = sx(0)
            for (let i = 0; i < n; i++) {
                ctx.fillStyle = (s.values && s.values[i] < 0) ? Theme.seriesColor(1).toString() : col
                const X = sx(s.x[i]), Y = sy(s.y[i])
                ctx.fillRect(Math.min(base, X), Y - bh / 2, Math.abs(X - base), bh)
            }
            break
        }
        case "hline": {
            ctx.lineWidth = 1.2
            ctx.beginPath()
            for (const y of s.y) { const Y = Math.round(sy(y)) + 0.5; ctx.moveTo(pl, Y); ctx.lineTo(pl + pw, Y) }
            ctx.stroke()
            break
        }
        case "vline": {
            ctx.lineWidth = 1.2
            ctx.beginPath()
            for (const x of s.x) { if (x === null || !isFinite(x)) continue; const X = Math.round(sx(x)) + 0.5; ctx.moveTo(X, pt); ctx.lineTo(X, pt + ph) }
            ctx.stroke()
            break
        }
        case "segments": {
            ctx.lineWidth = root.plot.kind === "3d" ? 1 : 1.1
            ctx.beginPath()
            for (let i = 0; i + 1 < n; i += 2) { ctx.moveTo(sx(s.x[i]), sy(s.y[i])); ctx.lineTo(sx(s.x[i + 1]), sy(s.y[i + 1])) }
            ctx.stroke()
            break
        }
        case "labels": {
            ctx.setLineDash([])
            ctx.font = root.font(fs, false)
            ctx.textAlign = "left"
            ctx.textBaseline = "bottom"
            for (let i = 0; i < n; i++) {
                const X = sx(s.x[i]) + 4, Y = sy(s.y[i]) - 2
                const t = s.labels[i]
                if (root.plot.heatmap && root.plot.kind !== "matrix") {
                    const tw = ctx.measureText(t).width
                    ctx.fillStyle = Theme.base.toString()
                    ctx.globalAlpha = 0.75
                    ctx.fillRect(X - 2, Y - fs - 1, tw + 4, fs + 3)
                    ctx.globalAlpha = 1
                    ctx.fillStyle = Theme.text.toString()
                } else {
                    ctx.fillStyle = col
                }
                ctx.fillText(t, X, Y)
            }
            break
        }
        case "polygons": {
            const lo = vr ? vr[0] : 0, span = vr ? (vr[1] - vr[0]) || 1 : 1
            ctx.lineWidth = 0.4
            ctx.strokeStyle = Theme.dark ? "rgba(0,0,0,0.45)" : "rgba(0,0,0,0.35)"
            for (let k = 0; k < s.values.length; k++) {
                ctx.fillStyle = Theme.heat((s.values[k] - lo) / span).toString()
                ctx.beginPath()
                for (let j = 0; j < 4; j++) {
                    const X = sx(s.x[4 * k + j]), Y = sy(s.y[4 * k + j])
                    if (j === 0) ctx.moveTo(X, Y); else ctx.lineTo(X, Y)
                }
                ctx.closePath()
                ctx.fill()
                ctx.stroke()
            }
            break
        }
        }
        ctx.setLineDash([])
    }

    ToolTip {
        id: tip
        delay: 0
        timeout: 4000
    }

    MouseArea {
        id: mouse
        anchors.fill: canvas
        hoverEnabled: true
        acceptedButtons: Qt.LeftButton
        property real pressX: 0
        property real pressY: 0
        property real startAz: 0
        property real startEl: 0
        property bool dragging: false
        cursorShape: root.plot && root.plot.kind === "3d" ? (pressed ? Qt.ClosedHandCursor : Qt.OpenHandCursor) : Qt.ArrowCursor

        function nearest(mx, my) {
            let best = null, bd = 64
            for (const q of root._points) {
                const d = (q.x - mx) * (q.x - mx) + (q.y - my) * (q.y - my)
                if (d < bd) { bd = d; best = q }
            }
            return best
        }

        onPositionChanged: (m) => {
            if (pressed && root.plot && root.plot.kind === "3d") {
                dragging = true
                root.azimuth = (startAz + (m.x - pressX) * 0.6) % 360
                root.elevation = Math.max(2, Math.min(88, startEl + (m.y - pressY) * 0.4))
                rotateTimer.restart()
                return
            }
            const q = nearest(m.x, m.y)
            if (q) {
                let t = (q.id >= 0 ? qsTr("Run %1").arg(q.id) + "\n" : "") + (q.label ? q.label + "\n" : "")
                t += root.fmt(q.dx) + ", " + root.fmt(q.dy)
                tip.text = t
                tip.x = q.x + 10
                tip.y = q.y - 40
                tip.open()
            } else {
                tip.close()
            }
        }
        onExited: tip.close()
        onPressed: (m) => {
            pressX = m.x; pressY = m.y
            startAz = root.azimuth; startEl = root.elevation
            dragging = false
        }
        onReleased: (m) => {
            if (dragging) {
                rotateTimer.stop()
                root.rotated(root.azimuth, root.elevation)
                return
            }
            const q = nearest(m.x, m.y)
            if (q && q.id >= 0) root.pointClicked(q.id)
            const g = root._geom
            if (g && m.x >= g.pl && m.x <= g.pl + g.pw && m.y >= g.pt && m.y <= g.pt + g.ph) {
                const x = g.x0 + (m.x - g.pl) / g.pw * (g.x1 - g.x0)
                const y = g.y0 + (g.pt + g.ph - m.y) / g.ph * (g.y1 - g.y0)
                root.clicked(x, y)
            }
        }
    }

    Timer {
        id: rotateTimer
        interval: 90
        onTriggered: root.rotated(root.azimuth, root.elevation)
    }
}
