import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Sdoe

// Model building and analysis for one response.
Item {
    id: page
    property var responses: []
    property var factors: []
    property string rid: ""
    property var model: null
    property var fit: null
    property string fitError: ""
    property var current: ({})
    property var selectionLog: []
    property var autoTable: null
    property int seenRevision: -1
    property int seenEpoch: -1
    property bool envInteractions: false

    readonly property var modelFactors: page.model ? page.factors.filter(f => page.model.factors.indexOf(f.id) >= 0) : []
    readonly property var numericModelFactors: modelFactors.filter(f => !f.categorical)

    function reload() {
        const r = Api.call("responses.list", {})
        const f = Api.call("factors.list", {})
        const st = Api.call("project.state", {})
        if (!r || !f || !st) return
        const newProject = st.epoch !== page.seenEpoch
        page.seenEpoch = st.epoch
        if (newProject) {
            page.rid = ""
            page.current = {}
            page.selectionLog = []
            page.autoTable = null
            page.fit = null
            page.model = null
            page.fitError = ""
            page.envInteractions = false
            clearResults()
        }
        page.responses = r
        page.factors = f
        const cur = Object.assign({}, page.current)
        for (const x of f) if (cur[x.id] === undefined) cur[x.id] = x.center
        for (const id of Object.keys(cur)) if (!f.some(x => x.id === id)) delete cur[id]
        page.current = cur
        if (!r.find(x => x.id === page.rid)) page.rid = r.length ? r[0].id : ""
        refreshModel()
        page.seenRevision = Backend.revision
    }

    function clearResults() {
        equation.text = ""
        pa.plot = null; rn.plot = null; rf.plot = null; ro.plot = null; ck.plot = null; bc.plot = null
        pareto.plot = null; halfn.plot = null
        profiler.model = []
        profiler.prediction = null
        surfacePlot.plot = null
        interPlot.plot = null
        dataPlot.plot = null
        robust.result = null
        robust.error = ""
    }

    function refreshModel() {
        if (!page.rid) {
            page.model = null
            page.fit = null
            page.fitError = ""
            clearResults()
            syncControls()
            return
        }
        page.model = Api.call("model.get", { response: page.rid })
        fixSelectors()
        const res = Api.tryCall("analysis.fit", { response: page.rid })
        page.fit = res.ok ? res.result : null
        page.fitError = res.ok ? "" : res.error
        if (!res.ok) clearResults()
        syncControls()
        refreshEquation()
        refreshPlots()
    }

    function syncControls() {
        const ri = page.responses.findIndex(x => x.id === page.rid)
        if (ri >= 0) respBox.currentIndex = ri
        if (page.model) {
            if (!formula.activeFocus) formula.text = page.model.formula
            const ti = page.model.transforms.indexOf(page.model.transform)
            if (ti >= 0) transform.currentIndex = ti
        } else if (!formula.activeFocus) {
            formula.text = ""
        }
    }

    function refreshEquation() {
        if (!page.fit) { equation.text = ""; return }
        const r = Api.tryCall("analysis.equation", { response: page.rid, units: units.currentIndex === 0 ? "actual" : "coded", syntax: page.model.syntaxes[syntax.currentIndex][0] })
        equation.text = r.ok ? r.result.text : r.error
    }

    function plot(kind, extra) {
        const params = Object.assign({ response: page.rid, kind: kind, values: page.current }, extra || {})
        const r = Api.tryCall("analysis.plot", params)
        return r.ok ? r.result : { title: r.error, x: { min: 0, max: 1, ticks: [] }, y: { min: 0, max: 1, ticks: [] }, series: [], kind: "xy" }
    }

    function refreshPlots() {
        if (!page.rid) return
        const t = tabs.currentIndex
        if (t === 0) {
            if (!page.fit) return
            pa.plot = plot("pred_actual"); rn.plot = plot("resid_normal"); rf.plot = plot("resid_fitted")
            ro.plot = plot("resid_order"); ck.plot = plot("cooks"); bc.plot = plot("boxcox")
        } else if (t === 1) {
            if (!page.fit) return
            pareto.plot = plot("pareto"); halfn.plot = plot("halfnormal")
        } else if (t === 2) {
            if (!page.fit) return
            const r = Api.tryCall("analysis.plot", { response: page.rid, kind: "profiler", values: page.current })
            profiler.model = r.ok ? r.result.plots : []
            profiler.prediction = r.ok ? r.result.prediction : null
        } else if (t === 3) {
            if (!page.fit || cx.currentIndex < 0 || cy.currentIndex < 0) return
            const a = page.numericModelFactors[cx.currentIndex], b = page.numericModelFactors[cy.currentIndex]
            if (!a || !b || a.id === b.id) { surfacePlot.plot = null; return }
            surfacePlot.plot = plot(surface3d.checked ? "surface" : "contour", { x: a.id, y: b.id, azimuth: surfacePlot.azimuth, elevation: surfacePlot.elevation })
        } else if (t === 4) {
            if (!page.fit || ix.currentIndex < 0 || iy.currentIndex < 0) return
            const a = page.modelFactors[ix.currentIndex], b = page.modelFactors[iy.currentIndex]
            if (!a || !b || a.id === b.id) { interPlot.plot = null; return }
            interPlot.plot = plot("interaction", { x: a.id, y: b.id })
        } else if (t === 5) {
            if (dx.currentIndex < 0 || !page.factors[dx.currentIndex]) return
            dataPlot.plot = plot("data", { x: page.factors[dx.currentIndex].id })
        } else if (t === 6) {
            const r = Api.tryCall("analysis.robustness", { response: page.rid, sn: ["nominal", "larger", "smaller"][snType.currentIndex] })
            robust.result = r.ok ? r.result : null
            robust.error = r.ok ? "" : r.error
        }
    }

    function setFormula(text) {
        if (Api.call("model.set", { response: page.rid, formula: text, hierarchy: hierarchy.checked }) !== undefined) page.selectionLog = []
    }

    function setCurrent(fid, text) {
        const c = Object.assign({}, page.current)
        c[fid] = text
        page.current = c
        refreshPlots()
    }

    // Sets the two factor selectors of a pair to valid, distinct indices.
    function fixPair(a, b, n) {
        if (a.currentIndex < 0 || a.currentIndex >= n) a.currentIndex = 0
        if (b.currentIndex < 0 || b.currentIndex >= n || b.currentIndex === a.currentIndex) b.currentIndex = n > 1 ? (a.currentIndex === 0 ? 1 : 0) : 0
    }
    function fixSelectors() {
        fixPair(cx, cy, page.numericModelFactors.length)
        fixPair(ix, iy, page.modelFactors.length)
        if (dx.currentIndex < 0) dx.currentIndex = 0
    }

    function showTab(i) { tabs.currentIndex = i }
    function showSurface3d() { tabs.currentIndex = 3; surface3d.checked = true; refreshPlots() }

    onVisibleChanged: if (visible && seenRevision !== Backend.revision) reload()
    Connections {
        target: Backend
        function onChanged() { if (page.visible) page.reload() }
    }
    Component.onCompleted: reload()

    ColumnLayout {
        anchors.fill: parent
        anchors.margins: 8
        spacing: 6

        RowLayout {
            Layout.fillWidth: true
            FitComboBox {
                id: respBox
                Layout.preferredWidth: 160
                model: page.responses.map(r => r.name)
                currentIndex: Math.max(0, page.responses.findIndex(r => r.id === page.rid))
                onActivated: (i) => { page.rid = page.responses[i].id; page.selectionLog = []; page.autoTable = null; page.refreshModel() }
            }
            TextField {
                id: formula
                Layout.fillWidth: true
                font.family: "monospace"
                placeholderText: qsTr("Model formula, e.g. A + B + A*B + A^2")
                onAccepted: page.setFormula(text)
                ToolTip.visible: hovered && !activeFocus
                ToolTip.delay: 800
                ToolTip.text: qsTr("Terms joined by +. Products with *, powers with ^, (A+B+C)^2 for all interactions up to order 2, - to remove a term.")
            }
            Button { text: qsTr("Apply"); enabled: page.rid !== ""; onClicked: page.setFormula(formula.text) }
            Button {
                text: qsTr("Model")
                enabled: page.rid !== ""
                onClicked: modelMenu.popup()
                FitMenu {
                    id: modelMenu
                    MenuItem { text: qsTr("Linear"); onTriggered: Api.call("model.preset", { response: page.rid, preset: "linear", env_interactions: page.envInteractions }) }
                    MenuItem { text: qsTr("Two-factor interactions"); onTriggered: Api.call("model.preset", { response: page.rid, preset: "2fi", env_interactions: page.envInteractions }) }
                    MenuItem { text: qsTr("Quadratic"); onTriggered: Api.call("model.preset", { response: page.rid, preset: "quadratic", env_interactions: page.envInteractions }) }
                    MenuItem { text: qsTr("Cubic"); onTriggered: Api.call("model.preset", { response: page.rid, preset: "cubic", env_interactions: page.envInteractions }) }
                    MenuItem {
                        text: qsTr("Include environment × factor interactions")
                        checkable: true
                        checked: page.envInteractions
                        onToggled: page.envInteractions = checked
                    }
                    MenuSeparator {}
                    MenuItem {
                        text: qsTr("Stepwise selection (AICc)")
                        onTriggered: {
                            const r = Api.call("model.select", { response: page.rid, method: "aicc", pool: "current" })
                            if (r) page.selectionLog = r.steps
                        }
                    }
                    MenuItem {
                        text: qsTr("Backward elimination (p > 0.10)")
                        onTriggered: {
                            const r = Api.call("model.select", { response: page.rid, method: "backward", alpha: 0.10, pool: "current" })
                            if (r) page.selectionLog = r.steps
                        }
                    }
                    MenuItem {
                        text: qsTr("Find best model and transform")
                        onTriggered: Api.run("model.auto", { response: page.rid, env_interactions: true }, r => { page.autoTable = r.table })
                    }
                }
            }
            CheckBox {
                id: hierarchy
                text: qsTr("Hierarchical")
                checked: true
                ToolTip.visible: hovered
                ToolTip.delay: 600
                ToolTip.text: qsTr("Adds lower-order terms implied by the formula (A*B adds A and B)")
            }
            FitComboBox {
                id: transform
                Layout.preferredWidth: 130
                model: page.model ? page.model.transforms : []
                currentIndex: page.model ? Math.max(0, page.model.transforms.indexOf(page.model.transform)) : 0
                displayText: page.model && page.model.transforms.indexOf(page.model.transform) < 0 ? page.model.transform : currentText
                onActivated: Api.call("model.set", { response: page.rid, transform: currentText })
                ToolTip.visible: hovered
                ToolTip.delay: 600
                ToolTip.text: qsTr("Response transform")
            }
        }

        SplitView {
            Layout.fillWidth: true
            Layout.fillHeight: true

            ScrollView {
                id: left
                SplitView.preferredWidth: page.width * 0.44
                SplitView.minimumWidth: 320
                contentWidth: availableWidth
                clip: true

                ColumnLayout {
                    width: left.availableWidth - 8
                    spacing: 6

                    Label {
                        Layout.fillWidth: true
                        visible: page.fitError.length > 0
                        text: page.responses.length === 0 ? qsTr("Add a response on the Setup page.") : page.fitError
                        color: page.responses.length === 0 ? Theme.muted : Theme.error
                        wrapMode: Text.WordWrap
                    }
                    Repeater {
                        model: page.fit ? page.fit.warnings : []
                        delegate: Label {
                            required property string modelData
                            Layout.fillWidth: true
                            text: "⚠ " + modelData
                            color: Theme.warn
                            wrapMode: Text.WordWrap
                        }
                    }
                    GridLayout {
                        visible: page.fit !== null
                        columns: 4
                        columnSpacing: 14
                        rowSpacing: 2
                        Repeater {
                            model: page.fit ? page.fit.stats : []
                            delegate: RowLayout {
                                required property var modelData
                                spacing: 6
                                Label { text: modelData[0]; color: Theme.muted }
                                Label { text: modelData[1]; font.bold: true }
                            }
                        }
                    }
                    Repeater {
                        model: page.fit ? page.fit.canonical : []
                        delegate: Label {
                            required property string modelData
                            Layout.fillWidth: true
                            text: modelData
                            wrapMode: Text.WordWrap
                        }
                    }
                    Label { visible: page.fit !== null; text: qsTr("Coefficients (coded units)"); font.bold: true; Layout.topMargin: 6 }
                    ResultTable {
                        visible: page.fit !== null
                        Layout.fillWidth: true
                        Layout.preferredHeight: implicitHeight
                        table: page.fit ? page.fit.coefficients : null
                        rowActionText: qsTr("Remove term")
                        onRowAction: (row, tag) => Api.call("model.remove_term", { response: page.rid, term: tag })
                    }
                    Label { visible: page.fit !== null; text: qsTr("Analysis of variance"); font.bold: true; Layout.topMargin: 6 }
                    ResultTable {
                        visible: page.fit !== null
                        Layout.fillWidth: true
                        Layout.preferredHeight: implicitHeight
                        table: page.fit ? page.fit.anova : null
                        rowActionText: qsTr("Remove term")
                        onRowAction: (row, tag) => Api.call("model.remove_term", { response: page.rid, term: tag })
                    }
                    RowLayout {
                        visible: page.fit !== null
                        Layout.topMargin: 6
                        Label { text: qsTr("Equation"); font.bold: true }
                        Item { Layout.fillWidth: true }
                        FitComboBox {
                            id: units
                            model: [qsTr("Actual units"), qsTr("Coded units")]
                            onActivated: page.refreshEquation()
                        }
                        FitComboBox {
                            id: syntax
                            model: page.model ? page.model.syntaxes.map(s => s[1]) : []
                            onActivated: page.refreshEquation()
                        }
                        Button {
                            text: qsTr("Copy")
                            onClicked: { equation.selectAll(); equation.copy(); equation.deselect() }
                        }
                    }
                    TextArea {
                        id: equation
                        visible: page.fit !== null
                        Layout.fillWidth: true
                        readOnly: true
                        selectByMouse: true
                        wrapMode: TextEdit.Wrap
                        font.family: "monospace"
                    }
                    Label { visible: page.autoTable !== null; text: qsTr("Model search"); font.bold: true; Layout.topMargin: 6 }
                    ResultTable {
                        visible: page.autoTable !== null
                        Layout.fillWidth: true
                        Layout.preferredHeight: implicitHeight
                        table: page.autoTable
                    }
                    Label {
                        visible: page.selectionLog.length > 0
                        Layout.fillWidth: true
                        text: page.selectionLog.join("\n")
                        color: Theme.muted
                        font.family: "monospace"
                        wrapMode: Text.WordWrap
                    }
                }
            }

            ColumnLayout {
                SplitView.fillWidth: true
                spacing: 4
                TabBar {
                    id: tabs
                    Layout.fillWidth: true
                    onCurrentIndexChanged: page.refreshPlots()
                    TabButton { text: qsTr("Diagnostics"); width: implicitWidth }
                    TabButton { text: qsTr("Effects"); width: implicitWidth }
                    TabButton { text: qsTr("Profiler"); width: implicitWidth }
                    TabButton { text: qsTr("Surface"); width: implicitWidth }
                    TabButton { text: qsTr("Interaction"); width: implicitWidth }
                    TabButton { text: qsTr("Data"); width: implicitWidth }
                    TabButton { text: qsTr("Robustness"); width: implicitWidth }
                }
                StackLayout {
                    Layout.fillWidth: true
                    Layout.fillHeight: true
                    currentIndex: tabs.currentIndex

                    GridLayout {
                        columns: width > 900 ? 3 : 2
                        PlotView { id: pa; Layout.fillWidth: true; Layout.fillHeight: true; compact: true }
                        PlotView { id: rn; Layout.fillWidth: true; Layout.fillHeight: true; compact: true }
                        PlotView { id: rf; Layout.fillWidth: true; Layout.fillHeight: true; compact: true }
                        PlotView { id: ro; Layout.fillWidth: true; Layout.fillHeight: true; compact: true }
                        PlotView { id: ck; Layout.fillWidth: true; Layout.fillHeight: true; compact: true }
                        PlotView { id: bc; Layout.fillWidth: true; Layout.fillHeight: true; compact: true }
                    }

                    GridLayout {
                        columns: width > 700 ? 2 : 1
                        PlotView { id: pareto; Layout.fillWidth: true; Layout.fillHeight: true }
                        PlotView { id: halfn; Layout.fillWidth: true; Layout.fillHeight: true }
                    }

                    ColumnLayout {
                        spacing: 4
                        RowLayout {
                            Label {
                                text: profiler.prediction ? qsTr("Prediction %1").arg(profiler.prediction.y) : ""
                                font.bold: true
                                font.pointSize: Qt.application.font.pointSize * 1.2
                            }
                            Label {
                                text: profiler.prediction ? qsTr("95% CI %1 to %2   95% PI %3 to %4").arg(profiler.prediction.ci[0]).arg(profiler.prediction.ci[1]).arg(profiler.prediction.pi[0]).arg(profiler.prediction.pi[1]) : ""
                                color: Theme.muted
                            }
                            Item { Layout.fillWidth: true }
                            Button {
                                text: qsTr("Center")
                                onClicked: {
                                    const c = {}
                                    for (const f of page.factors) c[f.id] = f.center
                                    page.current = c
                                    page.refreshPlots()
                                }
                            }
                        }
                        ScrollView {
                            id: profScroll
                            Layout.fillWidth: true
                            Layout.fillHeight: true
                            contentWidth: availableWidth
                            clip: true
                            Flow {
                                width: profScroll.availableWidth
                                spacing: 6
                                Repeater {
                                    id: profiler
                                    property var prediction: null
                                    model: []
                                    delegate: ColumnLayout {
                                        id: pcell
                                        required property var modelData
                                        readonly property var factor: page.factors.find(f => f.id === modelData.note)
                                        width: Math.max(240, Math.min(340, (profScroll.availableWidth - 12) / 3))
                                        spacing: 2
                                        PlotView {
                                            Layout.fillWidth: true
                                            Layout.preferredHeight: 210
                                            compact: true
                                            plot: pcell.modelData
                                            onClicked: (x, y) => {
                                                const f = pcell.factor
                                                if (!f) return
                                                if (f.categorical) page.setCurrent(f.id, f.levels[Math.max(0, Math.min(f.levels.length - 1, Math.round(x)))])
                                                else page.setCurrent(f.id, String(Number(x.toPrecision(5))))
                                            }
                                        }
                                        FactorValueEditor {
                                            Layout.alignment: Qt.AlignHCenter
                                            Layout.preferredWidth: 130
                                            factor: pcell.factor
                                            value: pcell.factor ? page.current[pcell.factor.id] || "" : ""
                                            onEdited: (v) => page.setCurrent(pcell.factor.id, v)
                                        }
                                    }
                                }
                            }
                        }
                    }

                    ColumnLayout {
                        RowLayout {
                            FitComboBox { id: cx; model: page.numericModelFactors.map(f => f.symbol + ": " + f.name); onActivated: page.refreshPlots() }
                            FitComboBox { id: cy; model: page.numericModelFactors.map(f => f.symbol + ": " + f.name); onActivated: page.refreshPlots() }
                            CheckBox { id: surface3d; text: qsTr("3D"); onToggled: page.refreshPlots() }
                            Item { Layout.fillWidth: true }
                        }
                        PlotView {
                            id: surfacePlot
                            Layout.fillWidth: true
                            Layout.fillHeight: true
                            onRotated: page.refreshPlots()
                        }
                    }

                    ColumnLayout {
                        RowLayout {
                            FitComboBox { id: ix; model: page.modelFactors.map(f => f.symbol + ": " + f.name); onActivated: page.refreshPlots() }
                            FitComboBox { id: iy; model: page.modelFactors.map(f => f.symbol + ": " + f.name); onActivated: page.refreshPlots() }
                            Item { Layout.fillWidth: true }
                        }
                        PlotView { id: interPlot; Layout.fillWidth: true; Layout.fillHeight: true }
                    }

                    ColumnLayout {
                        RowLayout {
                            FitComboBox { id: dx; model: page.factors.map(f => f.symbol + ": " + f.name); currentIndex: 0; onActivated: page.refreshPlots() }
                            Item { Layout.fillWidth: true }
                        }
                        PlotView { id: dataPlot; Layout.fillWidth: true; Layout.fillHeight: true }
                    }

                    ColumnLayout {
                        id: robust
                        property var result: null
                        property string error: ""
                        RowLayout {
                            FitComboBox {
                                id: snType
                                model: [qsTr("Nominal is best"), qsTr("Larger is better"), qsTr("Smaller is better")]
                                onActivated: page.refreshPlots()
                            }
                            Label {
                                Layout.fillWidth: true
                                text: robust.result ? qsTr("Highest S/N: ") + robust.result.best.join(", ") : robust.error
                                color: robust.result ? Theme.text : Theme.muted
                                wrapMode: Text.WordWrap
                            }
                        }
                        ScrollView {
                            id: robScroll
                            Layout.fillWidth: true
                            Layout.fillHeight: true
                            contentWidth: availableWidth
                            clip: true
                            ColumnLayout {
                                width: robScroll.availableWidth
                                Flow {
                                    Layout.fillWidth: true
                                    spacing: 6
                                    Repeater {
                                        model: robust.result ? robust.result.plots : []
                                        delegate: PlotView {
                                            required property var modelData
                                            width: 260
                                            height: 200
                                            compact: true
                                            plot: modelData
                                        }
                                    }
                                }
                                ResultTable {
                                    Layout.fillWidth: true
                                    Layout.preferredHeight: implicitHeight
                                    table: robust.result ? robust.result.table : null
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
