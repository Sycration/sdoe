import QtQuick
import QtQuick.Controls
import Sdoe

// Read-only table of an engine result table ({columns, rows, tags, emphasis, numeric}).
Item {
    id: root
    property var table: null
    property bool selectable: false
    property int currentRow: -1
    property int maxVisibleRows: 1000
    signal rowClicked(int row, string tag)
    signal rowMenuRequested(int row, string tag)
    property string rowActionText: ""
    signal rowAction(int row, string tag)
    readonly property int rows: tv.rows

    implicitHeight: hh.implicitHeight + Math.min(rows, maxVisibleRows) * 24 + (tv.contentWidth > tv.width ? 14 : 2)
    implicitWidth: tv.contentWidth + 2

    ResultModel {
        id: rm
        source: root.table ? JSON.stringify(root.table) : ""
        onModelReset: {
            root.currentRow = -1
            tv.forceLayout()
        }
    }
    FontMetrics { id: fm }

    TextEdit {
        id: clip
        visible: false
        function copyText(t) { text = t; selectAll(); copy(); text = "" }
    }

    HorizontalHeaderView {
        id: hh
        anchors.left: tv.left
        anchors.right: tv.right
        anchors.top: parent.top
        syncView: tv
        clip: true
        delegate: Rectangle {
            id: hcell
            required property var display
            required property int column
            implicitHeight: 26
            implicitWidth: 60
            color: Theme.window
            Rectangle { anchors.bottom: parent.bottom; width: parent.width; height: 1; color: Theme.frame }
            Label {
                anchors.fill: parent
                anchors.leftMargin: 6
                anchors.rightMargin: 6
                text: hcell.display === undefined ? "" : hcell.display
                elide: Text.ElideRight
                verticalAlignment: Text.AlignVCenter
                horizontalAlignment: rm.data(rm.index(0, hcell.column), 260) === true ? Text.AlignRight : Text.AlignLeft
                font.bold: true
            }
        }
    }

    TableView {
        id: tv
        anchors.top: hh.bottom
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.bottom: parent.bottom
        clip: true
        model: rm
        boundsBehavior: Flickable.StopAtBounds
        interactive: contentHeight > height || contentWidth > width
        ScrollBar.horizontal: ScrollBar { policy: tv.contentWidth > tv.width ? ScrollBar.AlwaysOn : ScrollBar.AlwaysOff }
        ScrollBar.vertical: ScrollBar {}
        columnWidthProvider: c => Math.max(44, Math.min(420, rm.widthHint(c) * fm.averageCharacterWidth * 1.1 + 18))
        rowHeightProvider: () => 24

        delegate: Rectangle {
            id: cell
            required property int row
            required property int column
            required property var display
            required property string emphasis
            required property bool numeric
            implicitHeight: 24
            color: root.selectable && root.currentRow === row ? Theme.selection : (row % 2 ? Theme.alternateBase : Theme.base)
            Label {
                anchors.fill: parent
                anchors.leftMargin: 6
                anchors.rightMargin: 6
                text: cell.display === undefined ? "" : cell.display
                elide: Text.ElideRight
                verticalAlignment: Text.AlignVCenter
                horizontalAlignment: cell.numeric ? Text.AlignRight : Text.AlignLeft
                font.bold: cell.emphasis === "strong" || cell.emphasis === "good"
                color: cell.emphasis === "muted" ? Theme.muted : (cell.emphasis === "bad" ? Theme.error : Theme.text)
            }
            TapHandler {
                onTapped: {
                    if (root.selectable) root.currentRow = cell.row
                    root.rowClicked(cell.row, rm.tag(cell.row))
                }
            }
            TapHandler {
                acceptedButtons: Qt.RightButton
                onTapped: {
                    if (root.selectable) root.currentRow = cell.row
                    menu.row = cell.row
                    root.rowMenuRequested(cell.row, rm.tag(cell.row))
                    menu.popup()
                }
            }
        }
    }

    FitMenu {
        id: menu
        property int row: -1
        MenuItem {
            text: root.rowActionText
            visible: root.rowActionText.length > 0 && menu.row >= 0 && rm.tag(menu.row).length > 0
            height: visible ? implicitHeight : 0
            onTriggered: root.rowAction(menu.row, rm.tag(menu.row))
        }
        MenuItem { text: qsTr("Copy table"); onTriggered: clip.copyText(rm.tsv()) }
    }

    function tagOf(row) { return rm.tag(row) }
    function copyAll() { clip.copyText(rm.tsv()) }
}
