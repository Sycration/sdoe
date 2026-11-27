import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Sdoe

// Run sheet: data entry for factor settings, measured environment and responses.
Item {
    id: page
    property var state: ({})
    signal importRequested()
    signal exportRequested()

    function reload() {
        const s = Api.call("project.state", {})
        if (s) page.state = s
    }
    Connections {
        target: Backend
        function onChanged() { page.reload() }
    }
    Component.onCompleted: reload()

    function startEdit(row, column, seed) {
        sheet.setCurrent(row, column)
        sheet.beginEdit(row, column, seed)
    }

    ColumnLayout {
        anchors.fill: parent
        anchors.margins: 8
        spacing: 6

        RowLayout {
            Layout.fillWidth: true
            Button { text: qsTr("Randomize order"); onClicked: Api.call("runs.randomize", {}) }
            Button {
                text: qsTr("Sort")
                onClicked: sortMenu.popup()
                FitMenu {
                    id: sortMenu
                    MenuItem { text: qsTr("By run order"); onTriggered: Api.call("runs.sort", { by: "run" }) }
                    MenuItem { text: qsTr("By standard order"); onTriggered: Api.call("runs.sort", { by: "std" }) }
                }
            }
            Button {
                text: qsTr("Next run…")
                enabled: page.state.factors > 0
                onClicked: nextRun.open()
                ToolTip.visible: hovered
                ToolTip.delay: 500
                ToolTip.text: qsTr("Suggest settings for one more run given the current environmental conditions")
            }
            Item { Layout.fillWidth: true }
            Button { text: qsTr("Import…"); onClicked: page.importRequested() }
            Button { text: qsTr("Export…"); enabled: page.state.runs > 0; onClicked: page.exportRequested() }
        }

        SheetView {
            id: sheet
            sheet: "runs"
            Layout.fillWidth: true
            Layout.fillHeight: true
        }

        RowLayout {
            Layout.fillWidth: true
            ProgressBar {
                Layout.preferredWidth: 160
                from: 0
                to: Math.max(1, page.state.runs || 0)
                value: page.state.done || 0
            }
            Label {
                text: qsTr("%1 of %2 runs done").arg(page.state.done || 0).arg(page.state.runs || 0)
                color: Theme.muted
            }
            Item { Layout.fillWidth: true }
            Repeater {
                model: [[Theme.envAccent, qsTr("Environmental (measure)")], [Theme.noiseAccent, qsTr("Noise")], [Theme.respAccent, qsTr("Response")]]
                delegate: RowLayout {
                    required property var modelData
                    spacing: 4
                    Rectangle { width: 12; height: 4; color: modelData[0] }
                    Label { text: modelData[1]; color: Theme.muted; rightPadding: 10 }
                }
            }
        }
    }

    NextRunDialog { id: nextRun }
}
