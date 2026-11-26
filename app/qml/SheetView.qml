import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Sdoe

// Editable spreadsheet over an engine sheet ("factors", "responses", "runs").
// Typing starts editing; Enter/Tab commit and move; Delete clears;
// Ctrl+C / Ctrl+V exchange tab-separated text with other programs.
Item {
    id: root
    property string sheet
    property var cols: []
    property int editRow: -1
    property int editCol: -1
    property var editSeed: null
    readonly property alias view: table
    readonly property int rowCount: table.rows
    property bool showRowHeader: true
    property int seenEpoch: -1

    function refresh() {
        const st = Api.call("project.state", {})
        if (st && st.epoch !== root.seenEpoch) {
            root.seenEpoch = st.epoch
            root.editRow = -1
            root.editCol = -1
            root.editSeed = null
        }
        sm.refresh()
    }

    SheetModel {
        id: sm
        sheet: root.sheet
        onColumnsChanged: {
            root.cols = columns.length ? JSON.parse(columns) : []
            table.forceLayout()
        }
    }

    Connections {
        target: Backend
        function onChanged() { root.refresh() }
    }
    Component.onCompleted: root.refresh()

    FontMetrics { id: fm }

    function cellEditable(r, c) {
        return sm.data(table.index(r, c), 258) === true
    }

    function setCurrent(r, c) {
        const rows = table.rows, ncols = table.columns
        if (rows === 0 || ncols === 0) return
        r = Math.max(0, Math.min(rows - 1, r))
        c = Math.max(0, Math.min(ncols - 1, c))
        table.selectionModel.setCurrentIndex(table.index(r, c), ItemSelectionModel.ClearAndSelect)
        table.positionViewAtCell(Qt.point(c, r), TableView.Contain)
    }

    function beginEdit(r, c, seed) {
        if (r < 0 || c < 0 || !cellEditable(r, c)) return
        const info = root.cols[c] || {}
        if (info.editor === "choice" && seed !== null) {
            // In a level column, a typed character sets the first matching level.
            const s = seed.toLowerCase()
            const hit = (info.choices || []).find(x => x.toLowerCase().startsWith(s))
            if (hit !== undefined) {
                const res = Api.tryCall("sheet.set", { sheet: root.sheet, row: r, col: c, text: hit })
                if (!res.ok) Api.error(res.error)
                setCurrent(r + 1, c)
            }
            return
        }
        root.editSeed = seed
        root.editRow = r
        root.editCol = c
    }

    function commit(text, move) {
        if (root.editRow < 0) return
        const r = root.editRow, c = root.editCol
        root.editRow = -1
        root.editCol = -1
        root.editSeed = null
        if (text !== null && text !== sm.cellText(r, c)) {
            const res = Api.tryCall("sheet.set", { sheet: root.sheet, row: r, col: c, text: text })
            if (!res.ok) Api.error(res.error)
        }
        table.forceActiveFocus()
        if (move === "down") setCurrent(r + 1, c)
        else if (move === "right") setCurrent(r, c + 1)
        else if (move === "left") setCurrent(r, c - 1)
        else if (move === "up") setCurrent(r - 1, c)
        else setCurrent(r, c)
    }

    function selectionRect() {
        const idx = table.selectionModel.selectedIndexes
        if (idx.length === 0) {
            const r = table.currentRow, c = table.currentColumn
            return { top: r, left: c, bottom: r, right: c }
        }
        let t = 1e9, l = 1e9, b = -1, rt = -1
        for (let i = 0; i < idx.length; i++) {
            t = Math.min(t, idx[i].row); b = Math.max(b, idx[i].row)
            l = Math.min(l, idx[i].column); rt = Math.max(rt, idx[i].column)
        }
        return { top: t, left: l, bottom: b, right: rt }
    }

    function selectedRows() {
        const set = {}
        const idx = table.selectionModel.selectedIndexes
        for (let i = 0; i < idx.length; i++) set[idx[i].row] = true
        if (idx.length === 0 && table.currentRow >= 0) set[table.currentRow] = true
        const last = table.rows - 1
        return Object.keys(set).map(Number).filter(r => r < last).sort((a, b) => a - b)
    }

    function copySelection() {
        const s = selectionRect()
        if (s.top < 0) return
        const res = Api.call("sheet.copy", { sheet: root.sheet, top: s.top, left: s.left, bottom: s.bottom, right: s.right })
        if (res) clip.copyText(res.text)
    }

    function pasteClipboard() {
        const t = clip.pasteText()
        if (!t) return
        const r = Math.max(0, table.currentRow), c = Math.max(0, table.currentColumn)
        const res = Api.call("sheet.paste", { sheet: root.sheet, row: r, col: c, text: t })
        if (res && res.errors.length > 0) {
            Api.error(res.errors.slice(0, 6).join("\n") + (res.errors.length > 6 ? "\n…" : ""))
        }
    }

    function clearSelection() {
        const s = selectionRect()
        if (s.top < 0) return
        const line = new Array(s.right - s.left + 1).fill("").join("\t")
        const text = new Array(s.bottom - s.top + 1).fill(line).join("\n")
        Api.call("sheet.paste", { sheet: root.sheet, row: s.top, col: s.left, text: text === "" ? " " : text })
    }

    function deleteRows() {
        const rows = selectedRows()
        if (rows.length > 0) Api.call("sheet.delete_rows", { sheet: root.sheet, rows: rows })
    }

    function insertRow() {
        Api.call("sheet.insert_row", { sheet: root.sheet, row: Math.max(0, table.currentRow) })
    }

    function setStatus(status) {
        Api.call("runs.set_status", { rows: selectedRows(), status: status })
    }

    // Clipboard access through an invisible text editor.
    TextEdit {
        id: clip
        visible: false
        function copyText(t) { text = t; selectAll(); copy(); text = "" }
        function pasteText() { text = ""; paste(); const t = text; text = ""; return t }
    }

    HorizontalHeaderView {
        id: hh
        anchors.left: table.left
        anchors.right: table.right
        anchors.top: parent.top
        syncView: table
        clip: true
        delegate: Rectangle {
            id: hcell
            required property var display
            required property int column
            readonly property var info: root.cols[column] || ({})
            implicitHeight: 28
            implicitWidth: 80
            color: Theme.window
            Rectangle { anchors.right: parent.right; width: 1; height: parent.height; color: Theme.grid }
            Rectangle {
                anchors.bottom: parent.bottom
                width: parent.width
                height: 3
                color: Theme.accentForKind(hcell.info.kind)
            }
            Label {
                anchors.fill: parent
                anchors.leftMargin: 6
                anchors.rightMargin: 6
                text: hcell.display === undefined ? "" : hcell.display
                elide: Text.ElideRight
                verticalAlignment: Text.AlignVCenter
                horizontalAlignment: hcell.info.numeric ? Text.AlignRight : Text.AlignLeft
                font.bold: true
            }
            HoverHandler { id: hhover }
            ToolTip.visible: hhover.hovered && (hcell.info.tip || "").length > 0
            ToolTip.text: hcell.info.tip || ""
            ToolTip.delay: 600
        }
    }

    VerticalHeaderView {
        id: vh
        visible: root.showRowHeader
        width: visible ? 36 : 0
        anchors.top: table.top
        anchors.bottom: table.bottom
        anchors.left: parent.left
        syncView: table
        clip: true
        delegate: Rectangle {
            id: vcell
            required property var display
            required property int row
            implicitWidth: 36
            implicitHeight: 26
            color: Theme.window
            Rectangle { anchors.bottom: parent.bottom; width: parent.width; height: 1; color: Theme.grid }
            Label {
                anchors.centerIn: parent
                text: vcell.display === undefined ? "" : vcell.display
                color: Theme.muted
            }
            TapHandler {
                onTapped: {
                    table.forceActiveFocus()
                    table.selectionModel.select(table.index(vcell.row, 0), ItemSelectionModel.ClearAndSelect | ItemSelectionModel.Rows)
                    table.selectionModel.setCurrentIndex(table.index(vcell.row, 0), ItemSelectionModel.NoUpdate)
                }
            }
        }
    }

    TableView {
        id: table
        anchors.left: vh.right
        anchors.top: hh.bottom
        anchors.right: parent.right
        anchors.bottom: parent.bottom
        clip: true
        model: sm
        focus: true
        keyNavigationEnabled: true
        pointerNavigationEnabled: true
        resizableColumns: true
        boundsBehavior: Flickable.StopAtBounds
        selectionBehavior: TableView.SelectCells
        selectionMode: TableView.ExtendedSelection
        selectionModel: ItemSelectionModel {}
        ScrollBar.horizontal: ScrollBar {}
        ScrollBar.vertical: ScrollBar {}

        columnWidthProvider: function (c) {
            const w = explicitColumnWidth(c)
            if (w >= 0) return w
            const ci = root.cols[c]
            if (!ci) return 100
            if (ci.kind === "meta" && ci.editor === "none") return ci.title === "Type" ? 96 : 58
            const titleW = fm.advanceWidth(ci.title) + 26
            return Math.max(ci.kind === "text" ? 110 : 84, Math.min(280, titleW))
        }
        rowHeightProvider: () => 26

        Keys.onPressed: (event) => {
            if (root.editRow >= 0) return
            const r = table.currentRow, c = table.currentColumn
            if (event.matches(StandardKey.Copy)) { root.copySelection(); event.accepted = true; return }
            if (event.matches(StandardKey.Paste)) { root.pasteClipboard(); event.accepted = true; return }
            if (event.matches(StandardKey.Cut)) { root.copySelection(); root.clearSelection(); event.accepted = true; return }
            if (event.matches(StandardKey.SelectAll)) {
                table.selectionModel.select(table.index(0, 0), ItemSelectionModel.Select)
                event.accepted = false
                return
            }
            if (event.key === Qt.Key_Delete || event.key === Qt.Key_Backspace) { root.clearSelection(); event.accepted = true; return }
            if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter || event.key === Qt.Key_F2) {
                root.beginEdit(r, c, null); event.accepted = true; return
            }
            if (event.key === Qt.Key_Tab) { root.setCurrent(r, c + 1); event.accepted = true; return }
            if (event.key === Qt.Key_Backtab) { root.setCurrent(r, c - 1); event.accepted = true; return }
            const mods = event.modifiers & (Qt.ControlModifier | Qt.AltModifier | Qt.MetaModifier)
            if (!mods && event.text.length === 1 && event.text >= " " && event.text !== "\u007f") {
                root.beginEdit(r, c, event.text)
                event.accepted = true
            }
        }

        delegate: Rectangle {
            id: cell
            required property int row
            required property int column
            required property var display
            required property string cellState
            required property bool editable
            required property bool selected
            required property bool current
            readonly property var info: root.cols[column] || ({})
            readonly property bool editing: root.editRow === row && root.editCol === column

            implicitHeight: 26
            color: {
                if (selected && !editing) return Theme.selection
                if (cellState === "needed") return Theme.needTint
                if (cellState === "placeholder") return Theme.base
                const t = Theme.tintForKind(info.kind)
                if (t !== "transparent") return t
                return row % 2 ? Theme.alternateBase : Theme.base
            }
            Rectangle { anchors.right: parent.right; width: 1; height: parent.height; color: Theme.grid }
            Rectangle { anchors.bottom: parent.bottom; width: parent.width; height: 1; color: Theme.grid }
            Rectangle {
                anchors.fill: parent
                color: "transparent"
                visible: cell.current && !cell.editing
                border.color: Theme.highlight
                border.width: 2
            }
            Label {
                anchors.fill: parent
                anchors.leftMargin: 6
                anchors.rightMargin: 6
                visible: !cell.editing
                text: cell.display === undefined ? "" : cell.display
                elide: Text.ElideRight
                verticalAlignment: Text.AlignVCenter
                horizontalAlignment: cell.info.numeric ? Text.AlignRight : Text.AlignLeft
                color: {
                    switch (cell.cellState) {
                    case "excluded": return Theme.disabledText
                    case "warn": return Theme.warn
                    case "error": return Theme.error
                    case "readonly": return Theme.muted
                    default: return Theme.text
                    }
                }
                font.strikeout: cell.cellState === "excluded" && cell.info.kind === "response"
            }
            Loader {
                anchors.fill: parent
                active: cell.editing
                sourceComponent: cell.info.editor === "choice" ? choiceEditor : (cell.info.editor === "combo" ? comboEditor : textEditor)
            }
            TapHandler {
                acceptedButtons: Qt.LeftButton
                gesturePolicy: TapHandler.DragThreshold
                onDoubleTapped: root.beginEdit(cell.row, cell.column, null)
            }
            TapHandler {
                acceptedButtons: Qt.RightButton
                onTapped: {
                    if (!cell.selected) root.setCurrent(cell.row, cell.column)
                    menu.popup()
                }
            }
        }
    }

    Component {
        id: textEditor
        TextField {
            id: tf
            text: root.editSeed !== null ? root.editSeed : sm.cellText(root.editRow, root.editCol)
            horizontalAlignment: (root.cols[root.editCol] || {}).numeric ? Text.AlignRight : Text.AlignLeft
            padding: 4
            Component.onCompleted: {
                forceActiveFocus()
                if (root.editSeed !== null) cursorPosition = text.length
                else selectAll()
            }
            Keys.onReturnPressed: root.commit(text, "down")
            Keys.onEnterPressed: root.commit(text, "down")
            Keys.onTabPressed: root.commit(text, "right")
            Keys.onBacktabPressed: root.commit(text, "left")
            Keys.onEscapePressed: root.commit(null, "")
            Keys.onUpPressed: root.commit(text, "up")
            Keys.onDownPressed: root.commit(text, "down")
            onActiveFocusChanged: if (!activeFocus && root.editRow >= 0) { const t = text; Qt.callLater(() => root.commit(t, "")) }
        }
    }

    Component {
        id: choiceEditor
        ComboBox {
            id: cb
            readonly property var choices: (root.cols[root.editCol] || {}).choices || []
            property bool ready: false
            model: choices
            Component.onCompleted: {
                const cur = sm.cellText(root.editRow, root.editCol)
                currentIndex = Math.max(0, choices.indexOf(cur))
                forceActiveFocus()
                Qt.callLater(() => { cb.ready = true; cb.popup.open() })
            }
            onActivated: root.commit(currentText, "")
            popup.onClosed: if (cb.ready && root.editRow >= 0) Qt.callLater(() => root.commit(null, ""))
            Keys.onEscapePressed: root.commit(null, "")
        }
    }

    Component {
        id: comboEditor
        ComboBox {
            id: ecb
            readonly property var choices: (root.cols[root.editCol] || {}).choices || []
            editable: true
            model: choices
            Component.onCompleted: {
                editText = root.editSeed !== null ? root.editSeed : sm.cellText(root.editRow, root.editCol)
                forceActiveFocus()
                if (contentItem && contentItem.cursorPosition !== undefined) contentItem.cursorPosition = editText.length
            }
            onAccepted: root.commit(editText, "down")
            onActivated: root.commit(currentText, "")
            Keys.onEscapePressed: root.commit(null, "")
            Keys.onTabPressed: root.commit(editText, "right")
            onActiveFocusChanged: if (!activeFocus && !popup.visible && root.editRow >= 0) Qt.callLater(() => root.commit(editText, ""))
        }
    }

    Menu {
        id: menu
        MenuItem { text: qsTr("Copy"); onTriggered: root.copySelection() }
        MenuItem { text: qsTr("Paste"); onTriggered: root.pasteClipboard() }
        MenuItem { text: qsTr("Clear"); onTriggered: root.clearSelection() }
        MenuSeparator {}
        MenuItem { text: qsTr("Insert row"); onTriggered: root.insertRow() }
        MenuItem { text: qsTr("Delete rows"); onTriggered: root.deleteRows() }
        MenuSeparator { visible: root.sheet === "runs"; height: visible ? implicitHeight : 0 }
        MenuItem { text: qsTr("Mark planned"); visible: root.sheet === "runs"; height: visible ? implicitHeight : 0; onTriggered: root.setStatus("Planned") }
        MenuItem { text: qsTr("Mark done"); visible: root.sheet === "runs"; height: visible ? implicitHeight : 0; onTriggered: root.setStatus("Done") }
        MenuItem { text: qsTr("Exclude from analysis"); visible: root.sheet === "runs"; height: visible ? implicitHeight : 0; onTriggered: root.setStatus("Excluded") }
    }
}
