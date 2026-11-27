import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Sdoe

// Desirability optimisation with fixed or random environmental and noise
// factors, solution profiles and environment-to-setting adjustment laws.
Item {
    id: page
    property var info: null
    property var env: ({})
    property var fixed: ({})
    property var result: null
    property int selected: 0
    property var profile: null
    property var law: null
    property int seenRevision: -1
    property int seenEpoch: -1

    readonly property var numericEnv: page.info ? page.info.env.filter(e => !e.categorical && e.role === "Environmental") : []

    function reload() {
        const i = Api.call("optimize.get", {})
        const st = Api.call("project.state", {})
        if (!i || !st) return
        if (st.epoch !== page.seenEpoch) {
            page.seenEpoch = st.epoch
            page.result = null
            page.profile = null
            page.law = null
            page.selected = 0
            tabs.currentIndex = 0
            lawFactor.currentIndex = 0
        }
        page.info = i
        const e = {}
        for (const x of i.env) e[x.id] = { mode: x.mode, value: x.value, mean: x.mean, sd: x.sd }
        page.env = e
        const f = {}
        for (const x of i.fixed) if (x.value !== "") f[x.id] = x.value
        page.fixed = f
        page.seenRevision = Backend.revision
    }

    function settings() {
        return { env: page.env, fixed: page.fixed }
    }

    function optimize() {
        const p = settings()
        p.solutions = 8
        Api.run("optimize.run", p, r => {
            page.result = r
            page.selected = 0
            page.showProfile()
            tabs.currentIndex = 0
        })
    }

    function showProfile() {
        if (!page.result || page.result.solutions.length === 0) { page.profile = null; return }
        const vals = page.result.solutions[page.selected].values
        const r = Api.tryCall("optimize.profile", { values: vals })
        page.profile = r.ok ? r.result : null
    }

    function computeLaw() {
        const f = page.numericEnv[lawFactor.currentIndex]
        if (!f) return
        Api.call("optimize.set", settings())
        Api.run("optimize.control_law", { factor: f.id }, r => { page.law = r; tabs.currentIndex = 1 })
    }

    function setEnv(id, key, value) {
        const e = Object.assign({}, page.env)
        e[id] = Object.assign({}, e[id])
        e[id][key] = value
        page.env = e
    }

    onVisibleChanged: if (visible && seenRevision !== Backend.revision) reload()
    Connections {
        target: Backend
        function onChanged() { if (page.visible) page.reload() }
    }
    Component.onCompleted: reload()

    SplitView {
        anchors.fill: parent
        anchors.margins: 8

        ScrollView {
            id: left
            SplitView.preferredWidth: 380
            SplitView.minimumWidth: 300
            contentWidth: availableWidth
            clip: true

            ColumnLayout {
                width: left.availableWidth - 8
                spacing: 6

                Label { text: qsTr("Goals"); font.bold: true }
                Repeater {
                    model: page.info ? page.info.goals : []
                    delegate: Label {
                        required property var modelData
                        Layout.fillWidth: true
                        elide: Text.ElideRight
                        color: modelData.goal === "None" ? Theme.muted : Theme.text
                        text: modelData.name + ": " + modelData.goal
                              + (modelData.goal !== "None" && (modelData.lower !== null || modelData.upper !== null)
                                 ? "  [" + (modelData.lower !== null ? modelData.lower : "…") + (modelData.goal === "Target" && modelData.target !== null ? " → " + modelData.target : "") + " – " + (modelData.upper !== null ? modelData.upper : "…") + "]" : "")
                    }
                }

                Label {
                    visible: page.info && page.info.env.length > 0
                    text: qsTr("Environment and noise")
                    font.bold: true
                    Layout.topMargin: 8
                }
                Repeater {
                    model: page.info ? page.info.env : []
                    delegate: ColumnLayout {
                        id: ecell
                        required property var modelData
                        readonly property var s: page.env[modelData.id] || {}
                        Layout.fillWidth: true
                        spacing: 2
                        RowLayout {
                            Layout.fillWidth: true
                            Label {
                                text: ecell.modelData.name
                                Layout.fillWidth: true
                                elide: Text.ElideRight
                                color: ecell.modelData.role === "Noise" ? Theme.noiseAccent : Theme.envAccent
                                font.bold: true
                            }
                            FitComboBox {
                                model: [qsTr("Fixed"), qsTr("Random")]
                                currentIndex: ecell.s.mode === "random" ? 1 : 0
                                onActivated: (i) => page.setEnv(ecell.modelData.id, "mode", i === 1 ? "random" : "fixed")
                            }
                        }
                        FactorValueEditor {
                            visible: ecell.s.mode !== "random"
                            Layout.fillWidth: true
                            factor: ecell.modelData
                            value: ecell.s.value || ""
                            onEdited: (v) => page.setEnv(ecell.modelData.id, "value", v)
                        }
                        RowLayout {
                            visible: ecell.s.mode === "random" && !ecell.modelData.categorical
                            Layout.fillWidth: true
                            Label { text: qsTr("Mean") }
                            TextField {
                                Layout.fillWidth: true
                                horizontalAlignment: Text.AlignRight
                                text: ecell.s.mean || ""
                                onEditingFinished: page.setEnv(ecell.modelData.id, "mean", text)
                            }
                            Label { text: qsTr("SD") }
                            TextField {
                                Layout.fillWidth: true
                                horizontalAlignment: Text.AlignRight
                                text: ecell.s.sd || ""
                                onEditingFinished: page.setEnv(ecell.modelData.id, "sd", text)
                            }
                        }
                    }
                }

                Label {
                    visible: page.info && page.info.fixed.length > 0
                    text: qsTr("Hold factors")
                    font.bold: true
                    Layout.topMargin: 8
                }
                Repeater {
                    model: page.info ? page.info.fixed : []
                    delegate: RowLayout {
                        id: fcell
                        required property var modelData
                        Layout.fillWidth: true
                        CheckBox {
                            id: hold
                            text: fcell.modelData.name
                            checked: page.fixed[fcell.modelData.id] !== undefined
                            Layout.fillWidth: true
                            onToggled: {
                                const f = Object.assign({}, page.fixed)
                                if (checked) f[fcell.modelData.id] = val.value || (fcell.modelData.categorical ? fcell.modelData.levels[0] : "")
                                else delete f[fcell.modelData.id]
                                page.fixed = f
                            }
                        }
                        FactorValueEditor {
                            id: val
                            enabled: hold.checked
                            Layout.preferredWidth: 120
                            factor: fcell.modelData
                            value: page.fixed[fcell.modelData.id] || ""
                            onEdited: (v) => {
                                const f = Object.assign({}, page.fixed)
                                f[fcell.modelData.id] = v
                                page.fixed = f
                            }
                        }
                    }
                }

                Button {
                    Layout.topMargin: 10
                    text: qsTr("Optimize")
                    highlighted: true
                    enabled: !Backend.busy && page.info !== null
                    onClicked: page.optimize()
                }

                Label {
                    visible: page.numericEnv.length > 0
                    text: qsTr("Adjustment law")
                    font.bold: true
                    Layout.topMargin: 12
                    ToolTip.visible: lawHover.hovered
                    ToolTip.delay: 400
                    ToolTip.text: qsTr("Best settings of the controlled factors as a function of an environmental factor, with fitted formulas")
                    HoverHandler { id: lawHover }
                }
                RowLayout {
                    visible: page.numericEnv.length > 0
                    FitComboBox {
                        id: lawFactor
                        Layout.fillWidth: true
                        model: page.numericEnv.map(e => e.name)
                    }
                    Button {
                        text: qsTr("Compute")
                        enabled: !Backend.busy
                        onClicked: page.computeLaw()
                    }
                }
            }
        }

        ColumnLayout {
            SplitView.fillWidth: true
            spacing: 4
            TabBar {
                id: tabs
                Layout.fillWidth: true
                TabButton { text: qsTr("Solutions"); width: implicitWidth }
                TabButton { text: qsTr("Adjustment law"); width: implicitWidth; enabled: page.law !== null }
            }
            StackLayout {
                Layout.fillWidth: true
                Layout.fillHeight: true
                currentIndex: tabs.currentIndex

                ColumnLayout {
                    spacing: 6
                    Repeater {
                        model: page.result ? page.result.notes : []
                        delegate: Label {
                            required property string modelData
                            Layout.fillWidth: true
                            text: modelData
                            wrapMode: Text.WordWrap
                            color: Theme.muted
                        }
                    }
                    ResultTable {
                        id: sols
                        Layout.fillWidth: true
                        Layout.preferredHeight: Math.min(implicitHeight, 240)
                        selectable: true
                        table: page.result ? page.result.table : null
                        onRowClicked: (row) => { page.selected = row; page.showProfile() }
                        Component.onCompleted: currentRow = 0
                    }
                    RowLayout {
                        visible: page.profile !== null
                        Label {
                            text: page.profile ? qsTr("Desirability %1").arg(page.profile.desirability.toFixed(4)) : ""
                            font.bold: true
                        }
                        Item { Layout.fillWidth: true }
                        Button {
                            text: qsTr("Add as confirmation run")
                            onClicked: {
                                const v = page.result.solutions[page.selected].values
                                if (Api.call("design.add_run", { values: v, type: "confirmation" }) !== undefined)
                                    Api.message(qsTr("Added a planned run on the Runs page."))
                            }
                        }
                    }
                    ScrollView {
                        id: profScroll
                        Layout.fillWidth: true
                        Layout.fillHeight: true
                        contentWidth: availableWidth
                        clip: true
                        GridLayout {
                            width: profScroll.availableWidth
                            columns: page.profile && page.profile.rows.length > 0 ? Math.max(1, page.profile.rows[0].length) : 1
                            rowSpacing: 0
                            columnSpacing: 0
                            Repeater {
                                model: page.profile ? [].concat.apply([], page.profile.rows) : []
                                delegate: PlotView {
                                    required property var modelData
                                    Layout.fillWidth: true
                                    Layout.preferredHeight: 150
                                    compact: true
                                    plot: modelData
                                    onClicked: (x, y) => {
                                        const vals = Object.assign({}, page.result.solutions[page.selected].values)
                                        const fid = modelData.note
                                        const f = Api.call("factors.list", {}).find(q => q.id === fid)
                                        if (!f) return
                                        vals[fid] = f.categorical ? f.levels[Math.max(0, Math.min(f.levels.length - 1, Math.round(x)))] : String(Number(x.toPrecision(5)))
                                        const r = Api.tryCall("optimize.profile", { values: vals })
                                        if (r.ok) {
                                            page.result.solutions[page.selected].values = vals
                                            page.profile = r.result
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                ScrollView {
                    id: lawScroll
                    contentWidth: availableWidth
                    clip: true
                    ColumnLayout {
                        width: lawScroll.availableWidth
                        spacing: 6
                        TextArea {
                            Layout.fillWidth: true
                            readOnly: true
                            selectByMouse: true
                            wrapMode: TextEdit.Wrap
                            font.family: "monospace"
                            text: page.law ? page.law.formulas.join("\n") : ""
                        }
                        Flow {
                            Layout.fillWidth: true
                            spacing: 6
                            Repeater {
                                model: page.law ? page.law.plots : []
                                delegate: PlotView {
                                    required property var modelData
                                    width: Math.max(300, (lawScroll.availableWidth - 12) / 2)
                                    height: 260
                                    plot: modelData
                                }
                            }
                        }
                        ResultTable {
                            Layout.fillWidth: true
                            Layout.preferredHeight: implicitHeight
                            table: page.law ? page.law.table : null
                        }
                    }
                }
            }
        }
    }
}
