import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Sdoe

// Design method selection, options, preview and application.
Item {
    id: page
    property var methods: []
    property var current: null
    property var preview: null
    property var savedOptions: ({})
    property int seenRevision: -1
    property int seenEpoch: -1
    signal applied()

    function reload() {
        const m = Api.call("design.methods", {})
        const st = Api.call("project.state", {})
        if (!m || !st) return
        if (st.epoch !== page.seenEpoch) {
            page.seenEpoch = st.epoch
            page.current = null
            page.savedOptions = {}
            page.preview = null
        }
        page.methods = m
        const id = page.current ? page.current.id : ""
        let sel = m.find(x => x.id === id && x.available) || m.find(x => x.available) || m[0]
        if (sel) select(sel)
        page.seenRevision = Backend.revision
    }

    function select(m) {
        if (page.current && page.current.id === m.id) {
            page.current = m
            form.specs = m.options
            return
        }
        if (page.current) savedOptions[page.current.id] = form.collect()
        page.current = m
        form.values = savedOptions[m.id] || {}
        form.specs = m.options
        page.preview = null
    }

    function runPreview() {
        if (!page.current) return
        const opts = form.collect()
        savedOptions[page.current.id] = opts
        Api.run("design.preview", { method: page.current.id, options: opts }, r => page.preview = r)
    }

    function apply(mode) {
        const res = Api.call("design.apply", { mode: mode, randomize: randomize.checked })
        if (res) page.applied()
    }

    onVisibleChanged: if (visible && seenRevision !== Backend.revision) reload()
    Connections {
        target: Backend
        function onChanged() {
            page.preview = null
            if (page.visible) page.reload()
        }
    }
    Component.onCompleted: reload()

    SplitView {
        anchors.fill: parent
        anchors.margins: 8

        ListView {
            id: list
            SplitView.preferredWidth: 230
            SplitView.minimumWidth: 160
            clip: true
            model: page.methods
            section.property: "group"
            section.delegate: Label {
                required property string section
                text: section
                font.bold: true
                color: Theme.muted
                topPadding: 8
                bottomPadding: 2
                leftPadding: 4
            }
            delegate: ItemDelegate {
                required property var modelData
                width: ListView.view.width
                text: modelData.name
                enabled: modelData.available
                highlighted: page.current && page.current.id === modelData.id
                onClicked: page.select(modelData)
                ToolTip.visible: hovered && !modelData.available
                ToolTip.text: modelData.reason
                ToolTip.delay: 300
            }
        }

        ScrollView {
            id: optScroll
            SplitView.preferredWidth: 420
            SplitView.minimumWidth: 300
            contentWidth: availableWidth
            ColumnLayout {
                width: optScroll.availableWidth
                spacing: 8
                Label {
                    text: page.current ? page.current.name : ""
                    font.bold: true
                    font.pointSize: Qt.application.font.pointSize * 1.25
                }
                Label {
                    Layout.fillWidth: true
                    text: page.current ? page.current.description : ""
                    wrapMode: Text.WordWrap
                    color: Theme.muted
                }
                OptionsForm {
                    id: form
                    Layout.fillWidth: true
                    Layout.topMargin: 6
                    onEdited: page.preview = null
                }
                RowLayout {
                    Layout.topMargin: 8
                    Button {
                        text: qsTr("Preview")
                        highlighted: !page.preview
                        enabled: page.current && page.current.available && !Backend.busy
                        onClicked: page.runPreview()
                    }
                    CheckBox {
                        id: randomize
                        text: qsTr("Randomize run order")
                        checked: true
                    }
                }
                RowLayout {
                    visible: page.preview !== null
                    Button {
                        text: page.preview && page.preview.augment ? qsTr("Add runs") : qsTr("Replace runs")
                        highlighted: true
                        onClicked: {
                            if (!page.preview.augment && Number(page.state().runs) > 0) confirmReplace.open()
                            else page.apply("replace")
                        }
                    }
                    Button {
                        visible: page.preview && !page.preview.augment
                        text: qsTr("Append to runs")
                        onClicked: page.apply("append")
                    }
                }
            }
        }

        ColumnLayout {
            SplitView.fillWidth: true
            spacing: 6
            TextArea {
                Layout.fillWidth: true
                Layout.preferredHeight: Math.min(implicitHeight, 220)
                readOnly: true
                visible: page.preview !== null
                wrapMode: TextEdit.Wrap
                font.family: "monospace"
                text: page.preview ? page.preview.notes.join("\n") + "\n\n" + qsTr("Model: ") + page.preview.model : ""
            }
            ResultTable {
                Layout.fillWidth: true
                Layout.fillHeight: true
                table: page.preview ? page.preview.table : null
            }
            Label {
                visible: page.preview === null
                Layout.fillWidth: true
                Layout.fillHeight: true
                horizontalAlignment: Text.AlignHCenter
                verticalAlignment: Text.AlignVCenter
                color: Theme.muted
                text: page.methods.length > 0 && page.methods.every(m => !m.available) ? qsTr("Add factors on the Setup page first.") : ""
            }
        }
    }

    function state() { return Api.call("project.state", {}) || {} }

    Dialog {
        id: confirmReplace
        anchors.centerIn: parent
        modal: true
        title: qsTr("Replace runs?")
        standardButtons: Dialog.Ok | Dialog.Cancel
        Label { text: qsTr("The current runs and their results will be removed (Undo restores them).") }
        onAccepted: page.apply("replace")
    }
}
