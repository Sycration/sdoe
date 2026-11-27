import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Sdoe

// Sequential experimentation: settings for the next run at today's
// environmental conditions, chosen for information or for best predicted outcome.
Dialog {
    id: dlg
    title: qsTr("Next run")
    modal: true
    anchors.centerIn: Overlay.overlay
    width: Math.min(620, Overlay.overlay ? Overlay.overlay.width - 40 : 620)
    height: Math.min(implicitHeight, Overlay.overlay ? Overlay.overlay.height - 40 : implicitHeight)
    standardButtons: Dialog.Close
    property var factors: []
    property var envValues: ({})
    property var suggestion: null
    property int seenEpoch: -1

    onAboutToShow: {
        const st = Api.call("project.state", {})
        if (st && st.epoch !== dlg.seenEpoch) {
            dlg.seenEpoch = st.epoch
            dlg.envValues = {}
        }
        const f = Api.call("factors.list", {})
        dlg.factors = f ? f.filter(x => x.role === "Environmental") : []
        const v = {}
        for (const x of dlg.factors) v[x.id] = dlg.envValues[x.id] !== undefined ? dlg.envValues[x.id] : x.center
        dlg.envValues = v
        dlg.suggestion = null
    }

    function suggest() {
        Api.run("design.next_run", { env: dlg.envValues, mode: best.checked ? "best" : "info" }, r => dlg.suggestion = r)
    }

    ColumnLayout {
        anchors.fill: parent
        spacing: 8
        ScrollView {
            id: envScroll
            visible: dlg.factors.length > 0
            Layout.fillWidth: true
            Layout.preferredHeight: Math.min(envGrid.implicitHeight, (Overlay.overlay ? Overlay.overlay.height : 600) * 0.4)
            contentWidth: availableWidth
            clip: true
            GridLayout {
                id: envGrid
                width: envScroll.availableWidth
                columns: 2
                Repeater {
                    model: dlg.factors
                    delegate: Label {
                        required property var modelData
                        required property int index
                        Layout.row: index
                        Layout.column: 0
                        text: modelData.name + (modelData.units ? " [" + modelData.units + "]" : "")
                    }
                }
                Repeater {
                    model: dlg.factors
                    delegate: FactorValueEditor {
                        required property var modelData
                        required property int index
                        Layout.row: index
                        Layout.column: 1
                        Layout.preferredWidth: 160
                        factor: modelData
                        value: dlg.envValues[modelData.id] || ""
                        onEdited: (v) => {
                            const e = Object.assign({}, dlg.envValues)
                            e[modelData.id] = v
                            dlg.envValues = e
                            dlg.suggestion = null
                        }
                    }
                }
            }
        }
        RowLayout {
            RadioButton { id: info; text: qsTr("Most informative"); checked: true }
            RadioButton { id: best; text: qsTr("Best predicted") }
            Item { Layout.fillWidth: true }
            Button { text: qsTr("Suggest"); highlighted: true; enabled: !Api.busy; onClicked: dlg.suggest() }
        }
        ResultTable {
            Layout.fillWidth: true
            Layout.preferredHeight: implicitHeight
            visible: dlg.suggestion !== null
            table: dlg.suggestion ? dlg.suggestion.table : null
        }
        Label {
            Layout.fillWidth: true
            visible: dlg.suggestion !== null && dlg.suggestion.predictions.length > 0
            text: dlg.suggestion ? dlg.suggestion.predictions.join("\n") : ""
            wrapMode: Text.WordWrap
            color: Theme.muted
        }
        Button {
            visible: dlg.suggestion !== null
            text: qsTr("Add as planned run")
            onClicked: {
                if (Api.call("design.add_run", { values: dlg.suggestion.values, type: best.checked ? "best" : "sequential" }) !== undefined) dlg.close()
            }
        }
    }
}
