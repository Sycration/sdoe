import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Sdoe

// Maps the columns of delimited text to factors, responses and run fields.
Dialog {
    id: dlg
    title: qsTr("Import data")
    modal: true
    anchors.centerIn: Overlay.overlay
    width: Math.min(760, Overlay.overlay ? Overlay.overlay.width - 40 : 760)
    height: Math.min(560, Overlay.overlay ? Overlay.overlay.height - 40 : 560)
    standardButtons: Dialog.Cancel
    property string text: ""
    property var preview: null
    property var mapping: []

    function load(t) {
        dlg.text = t
        const p = Api.call("import.preview", { text: t })
        if (!p) return
        dlg.preview = p
        dlg.mapping = p.columns.map(c => c.suggestion)
        open()
    }

    ColumnLayout {
        anchors.fill: parent
        spacing: 8
        Label {
            text: dlg.preview ? qsTr("%1 rows, %2 columns").arg(dlg.preview.rows).arg(dlg.preview.columns.length) : ""
            color: Theme.muted
        }
        ListView {
            Layout.fillWidth: true
            Layout.fillHeight: true
            clip: true
            model: dlg.preview ? dlg.preview.columns : []
            ScrollBar.vertical: ScrollBar {}
            delegate: RowLayout {
                id: row
                required property var modelData
                required property int index
                width: ListView.view.width
                spacing: 10
                Label {
                    text: row.modelData.name
                    font.bold: true
                    Layout.preferredWidth: 180
                    elide: Text.ElideRight
                }
                Label {
                    text: row.modelData.sample.join(", ")
                    color: Theme.muted
                    Layout.fillWidth: true
                    elide: Text.ElideRight
                }
                ComboBox {
                    Layout.preferredWidth: 260
                    model: dlg.preview.targets.map(t => t[1])
                    currentIndex: Math.max(0, dlg.preview.targets.findIndex(t => t[0] === dlg.mapping[row.index]))
                    onActivated: (i) => dlg.mapping[row.index] = dlg.preview.targets[i][0]
                }
            }
        }
        RowLayout {
            RadioButton { id: append; text: qsTr("Append runs"); checked: true }
            RadioButton { text: qsTr("Replace runs") }
            Item { Layout.fillWidth: true }
            Button {
                text: qsTr("Import")
                highlighted: true
                onClicked: {
                    const r = Api.call("import.apply", { text: dlg.text, mapping: dlg.mapping, mode: append.checked ? "append" : "replace" })
                    if (r) {
                        dlg.close()
                        Api.message(qsTr("Imported %1 runs.").arg(r.runs) + (r.messages.length ? " " + r.messages.join(" ") : ""))
                    }
                }
            }
        }
    }
}
