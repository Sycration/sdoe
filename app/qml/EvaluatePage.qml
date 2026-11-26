import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Sdoe

// Design evaluation: efficiency, prediction variance, correlations and power.
Item {
    id: page
    property var result: null
    property int seenRevision: -1

    function evaluate() {
        const p = { source: source.currentIndex === 0 ? "runs" : "preview", snr: snr.text }
        const presets = ["", "linear", "2fi", "quadratic", "cubic", ""]
        if (modelBox.currentIndex === 5) p.formula = formula.text
        else p.preset = presets[modelBox.currentIndex]
        p.env_interactions = envInt.checked
        const r = Api.tryCall("evaluate.run", p)
        page.result = r.ok ? r.result : null
        errorLabel.text = r.ok ? "" : r.error
        page.seenRevision = Backend.revision
    }

    onVisibleChanged: if (visible && seenRevision !== Backend.revision) evaluate()
    Connections {
        target: Backend
        function onChanged() { if (page.visible) page.evaluate() }
    }

    ColumnLayout {
        anchors.fill: parent
        anchors.margins: 8
        spacing: 6

        RowLayout {
            Layout.fillWidth: true
            ComboBox {
                id: source
                model: [qsTr("Current runs"), qsTr("Design preview")]
                onActivated: page.evaluate()
            }
            ComboBox {
                id: modelBox
                model: [qsTr("Default model"), qsTr("Linear"), qsTr("Two-factor interactions"), qsTr("Quadratic"), qsTr("Cubic"), qsTr("Custom formula")]
                onActivated: if (currentIndex !== 5) page.evaluate()
            }
            TextField {
                id: formula
                visible: modelBox.currentIndex === 5
                Layout.fillWidth: true
                font.family: "monospace"
                placeholderText: qsTr("Model formula")
                onAccepted: page.evaluate()
            }
            CheckBox {
                id: envInt
                text: qsTr("Environment interactions")
                visible: modelBox.currentIndex >= 2 && modelBox.currentIndex <= 4
                onToggled: page.evaluate()
            }
            Item { Layout.fillWidth: !formula.visible }
            Label { text: qsTr("Effect / σ") }
            TextField {
                id: snr
                text: "2"
                Layout.preferredWidth: 56
                horizontalAlignment: Text.AlignRight
                onEditingFinished: page.evaluate()
                ToolTip.visible: hovered
                ToolTip.delay: 500
                ToolTip.text: qsTr("Size of the effect to detect, in units of the error standard deviation, for the power column")
            }
            Button { text: qsTr("Evaluate"); onClicked: page.evaluate() }
        }

        Label {
            id: errorLabel
            Layout.fillWidth: true
            visible: text.length > 0
            color: Theme.error
            wrapMode: Text.WordWrap
        }

        SplitView {
            Layout.fillWidth: true
            Layout.fillHeight: true
            visible: page.result !== null

            ScrollView {
                id: left
                SplitView.preferredWidth: page.width * 0.42
                contentWidth: availableWidth
                clip: true
                ColumnLayout {
                    width: left.availableWidth - 8
                    spacing: 6
                    Label {
                        Layout.fillWidth: true
                        text: page.result ? page.result.formula : ""
                        font.family: "monospace"
                        wrapMode: Text.WrapAnywhere
                        color: Theme.muted
                    }
                    GridLayout {
                        columns: 2
                        columnSpacing: 14
                        rowSpacing: 2
                        Repeater {
                            model: page.result ? page.result.stats : []
                            delegate: RowLayout {
                                required property var modelData
                                Layout.columnSpan: 2
                                Label { text: modelData[0]; color: Theme.muted; Layout.preferredWidth: 220 }
                                Label { text: modelData[1]; font.bold: true }
                            }
                        }
                    }
                    Repeater {
                        model: page.result ? page.result.warnings : []
                        delegate: Label {
                            required property string modelData
                            Layout.fillWidth: true
                            text: "⚠ " + modelData
                            color: Theme.warn
                            wrapMode: Text.WordWrap
                        }
                    }
                    ResultTable {
                        Layout.fillWidth: true
                        Layout.preferredHeight: implicitHeight
                        table: page.result ? page.result.table : null
                    }
                }
            }

            ColumnLayout {
                SplitView.fillWidth: true
                PlotView {
                    Layout.fillWidth: true
                    Layout.fillHeight: true
                    Layout.preferredHeight: 3
                    plot: page.result ? page.result.correlation : null
                }
                PlotView {
                    Layout.fillWidth: true
                    Layout.fillHeight: true
                    Layout.preferredHeight: 2
                    plot: page.result ? page.result.fds : null
                }
            }
        }
    }
}
