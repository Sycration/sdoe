import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Sdoe

// Project title, factors and responses.
Item {
    id: page
    property var state: ({})
    property int seenEpoch: -1

    function reload() {
        const s = Api.call("project.state", {})
        if (!s) return
        const newProject = s.epoch !== page.seenEpoch
        page.state = s
        page.seenEpoch = s.epoch
        if (newProject || !title.activeFocus) title.text = s.title || ""
        if (newProject || !mixture.activeFocus) mixture.text = s.mixture_total !== undefined ? String(s.mixture_total) : "1"
        if (newProject || !notes.activeFocus) notes.text = s.notes || ""
    }
    Connections {
        target: Backend
        function onChanged() { page.reload() }
    }
    Component.onCompleted: reload()

    ColumnLayout {
        anchors.fill: parent
        anchors.margins: 8
        spacing: 6

        RowLayout {
            Layout.fillWidth: true
            TextField {
                id: title
                Layout.fillWidth: true
                placeholderText: qsTr("Project title")
                font.pointSize: Qt.application.font.pointSize * 1.2
                onEditingFinished: if (text !== (page.state.title || "")) Api.call("project.set", { title: text })
            }
            Label {
                visible: page.state.has_mixture === true
                text: qsTr("Mixture total")
            }
            TextField {
                id: mixture
                visible: page.state.has_mixture === true
                Layout.preferredWidth: 80
                horizontalAlignment: Text.AlignRight
                onEditingFinished: Api.call("project.set", { mixture_total: text })
            }
        }

        SplitView {
            Layout.fillWidth: true
            Layout.fillHeight: true
            orientation: Qt.Vertical
            handle: Rectangle {
                implicitHeight: 6
                color: SplitHandle.pressed || SplitHandle.hovered ? Theme.selection : Theme.frame
            }

            SplitView {
                SplitView.fillWidth: true
                SplitView.fillHeight: true
                SplitView.minimumHeight: 180
                orientation: Qt.Vertical

                ColumnLayout {
                    SplitView.fillHeight: true
                    SplitView.minimumHeight: 140
                    spacing: 2
                    Label { text: qsTr("Factors"); font.bold: true }
                    SheetView {
                        sheet: "factors"
                        Layout.fillWidth: true
                        Layout.fillHeight: true
                    }
                }
                ColumnLayout {
                    SplitView.preferredHeight: parent.height * 0.36
                    SplitView.minimumHeight: 110
                    spacing: 2
                    Label { text: qsTr("Responses"); font.bold: true; Layout.topMargin: 6 }
                    SheetView {
                        sheet: "responses"
                        Layout.fillWidth: true
                        Layout.fillHeight: true
                    }
                }
            }

            ColumnLayout {
                SplitView.preferredHeight: 84
                SplitView.minimumHeight: 48
                spacing: 2

                ScrollView {
                    id: notesScroll
                    Layout.fillWidth: true
                    Layout.fillHeight: true
                    clip: true

                    TextArea {
                        id: notes
                        width: notesScroll.availableWidth
                        placeholderText: qsTr("Notes")
                        wrapMode: TextEdit.Wrap
                        onActiveFocusChanged: if (!activeFocus && text !== (page.state.notes || "")) Api.call("project.set", { notes: text })
                    }
                }
            }
        }
    }
}
