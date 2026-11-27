import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import QtQuick.Dialogs
import QtCore
import Sdoe

ApplicationWindow {
    id: win
    width: 1320
    height: 860
    minimumWidth: 900
    minimumHeight: 560
    visible: true
    title: (fileUrl ? Backend.fileName(fileUrl) : qsTr("Untitled")) + (dirty ? " •" : "") + " — sdoe"

    property string fileUrl: ""
    property bool dirty: false
    property bool canUndo: false
    property bool canRedo: false
    property var pendingAction: null
    property bool quitting: false
    // In the browser, files are uploaded and downloaded instead.
    readonly property bool web: Backend.webFiles()

    function refreshState() {
        const s = Api.call("project.state", {})
        if (!s) return
        win.dirty = s.dirty
        win.canUndo = s.can_undo
        win.canRedo = s.can_redo
    }

    function showStatus(text, isError) {
        status.text = text
        status.color = isError ? Theme.error : Theme.text
        statusTimer.interval = isError ? 9000 : 5000
        statusTimer.restart()
    }

    // Commits the edit in the focused field: text fields send their value to
    // the engine when they lose focus, so focus moves away and comes back.
    function commitEdits() {
        Api.commitRequested()
        const item = win.activeFocusItem
        if (!item || item === focusSink) return
        focusSink.forceActiveFocus()
        if (item.visible) item.forceActiveFocus()
    }

    // Runs `action` after offering to save unsaved changes.
    function guard(action) {
        commitEdits()
        if (!win.dirty) { action(); return }
        win.pendingAction = action
        confirmDialog.open()
    }

    function runPending() {
        const a = win.pendingAction
        win.pendingAction = null
        if (a) a()
    }

    function folderOf(url) {
        const s = url.toString()
        return s.substring(0, s.lastIndexOf("/"))
    }

    function openFile(url) {
        const r = JSON.parse(Backend.readFile(url))
        if (!r.ok) { showStatus(r.error, true); return }
        if (openText(r.text, url.toString()))
            settings.lastFolder = folderOf(url)
    }

    // Opens project text; `name` is the file URL, or the file name in the browser.
    function openText(text, name) {
        if (Api.call("project.open", { text: text }) === undefined) return false
        win.fileUrl = name
        refreshState()
        showStatus(qsTr("Opened %1").arg(Backend.fileName(win.fileUrl)), false)
        return true
    }

    function showOpen() {
        if (web) Backend.openContent(qsTr("sdoe projects (*.sdoe)"), "project")
        else openDialog.open()
    }

    function showImport() {
        if (web) Backend.openContent(qsTr("Delimited text (*.csv *.tsv *.txt)"), "import")
        else importFileDialog.open()
    }

    // Exports with the dialog, or in the browser as a download named `name`.
    function showExport(dialog, method, key, name) {
        commitEdits()
        if (!web) { dialog.open(); return }
        const res = Api.call(method, {})
        if (!res) return
        Backend.saveContent(name, res[key])
    }

    // Saves to the current file (asking for a name if there is none); runs `then` on success.
    function save(then) {
        commitEdits()
        if (web) {
            const res = Api.call("project.save", {})
            if (!res) return
            if (!win.fileUrl) win.fileUrl = "untitled.sdoe"
            Backend.saveContent(win.fileUrl, res.text)
            Api.call("project.mark_saved", {})
            refreshState()
            if (then) then()
            return
        }
        if (!win.fileUrl) {
            saveAsDialog.after = then
            saveAsDialog.open()
            return
        }
        saveTo(win.fileUrl, then)
    }

    // Writes the project to `url`; the window takes that file name only when
    // the write succeeds. Runs `then` on success.
    function saveTo(url, then) {
        const res = Api.call("project.save", {})
        if (!res) { win.pendingAction = null; return }
        const err = Backend.writeFile(url, res.text)
        if (err) { showStatus(err, true); win.pendingAction = null; return }
        win.fileUrl = url
        Api.call("project.mark_saved", {})
        refreshState()
        showStatus(qsTr("Saved %1").arg(Backend.fileName(win.fileUrl)), false)
        if (then) then()
    }

    function exportText(url, method, key) {
        const res = Api.call(method, {})
        if (!res) return
        const err = Backend.writeFile(url, res[key])
        if (err) showStatus(err, true)
        else showStatus(qsTr("Exported %1").arg(Backend.fileName(url.toString())), false)
        settings.lastFolder = folderOf(url)
    }

    function loadExample(name) {
        guard(() => {
            if (Api.call("project.example", { name: name }) !== undefined) {
                win.fileUrl = ""
                tabs.currentIndex = 3
            }
        })
    }

    Connections {
        target: Backend
        function onChanged() { win.refreshState() }
        function onContentOpened(purpose, name, text) {
            if (purpose === "project") win.openText(text, name)
            else if (purpose === "import") importDialog.load(text)
        }
    }
    Connections {
        target: Api
        function onError(message) { win.showStatus(message, true) }
        function onMessage(text) { win.showStatus(text, false) }
    }

    Settings {
        id: settings
        property string lastFolder: ""
        property alias windowWidth: win.width
        property alias windowHeight: win.height
    }

    Component.onCompleted: {
        const f = Backend.startupFile()
        if (f) win.fileUrl = f
        refreshState()
        if (web) {
            // A browser tab cannot be quit: the last two entries of the File
            // menu (separator and Quit) go.
            fileMenu.removeItem(fileMenu.itemAt(fileMenu.count - 1))
            fileMenu.removeItem(fileMenu.itemAt(fileMenu.count - 1))
            win.showFullScreen()
        }
    }

    onClosing: (close) => {
        commitEdits()
        if (win.dirty && !win.quitting) {
            close.accepted = false
            win.pendingAction = () => { win.quitting = true; Qt.quit() }
            confirmDialog.open()
        }
    }

    menuBar: MenuBar {
        FitMenu {
            id: fileMenu
            title: qsTr("&File")
            Action { text: qsTr("&New"); shortcut: StandardKey.New; onTriggered: win.guard(() => { Api.call("project.new", {}); win.fileUrl = ""; tabs.currentIndex = 0 }) }
            Action { text: qsTr("&Open…"); shortcut: StandardKey.Open; onTriggered: win.guard(() => win.showOpen()) }
            FitMenu {
                title: qsTr("Open example")
                Action { text: qsTr("Reactor with measured humidity"); onTriggered: win.loadExample("reactor") }
                Action { text: qsTr("Polymer blend (mixture)"); onTriggered: win.loadExample("blend") }
            }
            MenuSeparator {}
            Action { text: qsTr("&Save"); shortcut: StandardKey.Save; onTriggered: win.save(null) }
            Action { text: qsTr("Save &As…"); shortcut: StandardKey.SaveAs; onTriggered: { win.commitEdits(); if (win.web) { win.save(null); return } saveAsDialog.after = null; saveAsDialog.open() } }
            MenuSeparator {}
            Action { text: qsTr("&Import data…"); onTriggered: win.showImport() }
            Action { text: qsTr("Import from clipboard"); onTriggered: { clip.text = ""; clip.paste(); const t = clip.text; clip.text = ""; if (t) importDialog.load(t) } }
            Action { text: qsTr("Export runs (CSV)…"); onTriggered: win.showExport(csvDialog, "export.csv", "text", "runs.csv") }
            Action { text: qsTr("Export report (HTML)…"); onTriggered: win.showExport(reportDialog, "export.report", "html", "report.html") }
            Action { text: qsTr("Export analysis (R)…"); onTriggered: win.showExport(rDialog, "export.r", "text", "analysis.R") }
            MenuSeparator {}
            Action { text: qsTr("&Quit"); shortcut: StandardKey.Quit; onTriggered: win.close() }
        }
        FitMenu {
            title: qsTr("&Edit")
            Action { text: qsTr("&Undo"); shortcut: StandardKey.Undo; enabled: win.canUndo; onTriggered: Api.call("edit.undo", {}) }
            Action { text: qsTr("&Redo"); shortcut: StandardKey.Redo; enabled: win.canRedo; onTriggered: Api.call("edit.redo", {}) }
        }
        FitMenu {
            title: qsTr("&View")
            Repeater {
                model: [qsTr("Setup"), qsTr("Design"), qsTr("Runs"), qsTr("Analysis"), qsTr("Optimize"), qsTr("Evaluate")]
                delegate: MenuItem {
                    required property string modelData
                    required property int index
                    text: modelData
                    onTriggered: tabs.currentIndex = index
                }
            }
        }
    }

    header: TabBar {
        id: tabs
        TabButton { text: qsTr("Setup") }
        TabButton { text: qsTr("Design") }
        TabButton { text: qsTr("Runs") }
        TabButton { text: qsTr("Analysis") }
        TabButton { text: qsTr("Optimize") }
        TabButton { text: qsTr("Evaluate") }
    }

    Repeater {
        model: 6
        delegate: Item {
            required property int index
            Shortcut {
                sequence: "Ctrl+" + (index + 1)
                onActivated: tabs.currentIndex = index
            }
        }
    }

    Rectangle {
        id: pages
        anchors.fill: parent
        color: Theme.window
    StackLayout {
        anchors.fill: parent
        currentIndex: tabs.currentIndex
        SetupPage {}
        DesignPage { id: designPage; onApplied: tabs.currentIndex = 2 }
        RunsPage {
            id: runsPage
            onImportRequested: win.showImport()
            onExportRequested: win.showExport(csvDialog, "export.csv", "text", "runs.csv")
        }
        AnalysisPage { id: analysisPage }
        OptimizePage { id: optimizePage }
        EvaluatePage {}
    }
    }

    footer: ToolBar {
        RowLayout {
            anchors.fill: parent
            anchors.leftMargin: 8
            anchors.rightMargin: 8
            Label {
                id: status
                Layout.fillWidth: true
                elide: Text.ElideRight
                maximumLineCount: 1
                ToolTip.visible: statusHover.hovered && text.length > 0
                ToolTip.text: text
                HoverHandler { id: statusHover }
            }
            BusyIndicator {
                running: Api.busy
                visible: running
                Layout.preferredHeight: 22
                Layout.preferredWidth: 22
            }
        }
    }

    Timer {
        id: statusTimer
        onTriggered: status.text = ""
    }

    // Modal overlay during background computations.
    Popup {
        modal: true
        visible: Api.busy
        closePolicy: Popup.NoAutoClose
        anchors.centerIn: Overlay.overlay
        background: Rectangle { color: "transparent" }
        Overlay.modal: Rectangle { color: Qt.rgba(0, 0, 0, 0.08) }
        BusyIndicator { running: true }
    }

    TextEdit { id: clip; visible: false }
    // Takes the focus for a moment in commitEdits().
    Item { id: focusSink; activeFocusOnTab: false }

    Dialog {
        id: confirmDialog
        anchors.centerIn: parent
        modal: true
        title: qsTr("Unsaved changes")
        standardButtons: Dialog.Save | Dialog.Discard | Dialog.Cancel
        Label { text: qsTr("Save changes to the current project?") }
        onAccepted: win.save(() => win.runPending())
        onDiscarded: win.runPending()
        onRejected: win.pendingAction = null
    }

    FileDialog {
        id: openDialog
        title: qsTr("Open project")
        nameFilters: [qsTr("sdoe projects (*.sdoe)"), qsTr("All files (*)")]
        currentFolder: settings.lastFolder ? settings.lastFolder : StandardPaths.writableLocation(StandardPaths.DocumentsLocation)
        onAccepted: win.openFile(selectedFile)
    }

    FileDialog {
        id: saveAsDialog
        property var after: null
        title: qsTr("Save project")
        fileMode: FileDialog.SaveFile
        defaultSuffix: "sdoe"
        nameFilters: [qsTr("sdoe projects (*.sdoe)")]
        currentFolder: settings.lastFolder ? settings.lastFolder : StandardPaths.writableLocation(StandardPaths.DocumentsLocation)
        onAccepted: {
            settings.lastFolder = win.folderOf(selectedFile)
            const a = after
            after = null
            win.commitEdits()
            win.saveTo(selectedFile.toString(), a)
        }
        onRejected: { after = null; win.pendingAction = null }
    }

    FileDialog {
        id: importFileDialog
        title: qsTr("Import data")
        nameFilters: [qsTr("Delimited text (*.csv *.tsv *.txt)"), qsTr("All files (*)")]
        currentFolder: settings.lastFolder ? settings.lastFolder : StandardPaths.writableLocation(StandardPaths.DocumentsLocation)
        onAccepted: {
            const r = JSON.parse(Backend.readFile(selectedFile))
            if (!r.ok) { win.showStatus(r.error, true); return }
            settings.lastFolder = win.folderOf(selectedFile)
            importDialog.load(r.text)
        }
    }

    FileDialog {
        id: csvDialog
        title: qsTr("Export runs")
        fileMode: FileDialog.SaveFile
        defaultSuffix: "csv"
        nameFilters: [qsTr("CSV (*.csv)")]
        currentFolder: settings.lastFolder ? settings.lastFolder : StandardPaths.writableLocation(StandardPaths.DocumentsLocation)
        onAccepted: win.exportText(selectedFile, "export.csv", "text")
    }

    FileDialog {
        id: reportDialog
        title: qsTr("Export report")
        fileMode: FileDialog.SaveFile
        defaultSuffix: "html"
        nameFilters: [qsTr("HTML (*.html)")]
        currentFolder: settings.lastFolder ? settings.lastFolder : StandardPaths.writableLocation(StandardPaths.DocumentsLocation)
        onAccepted: win.exportText(selectedFile, "export.report", "html")
    }

    FileDialog {
        id: rDialog
        title: qsTr("Export analysis")
        fileMode: FileDialog.SaveFile
        defaultSuffix: "R"
        nameFilters: [qsTr("R script (*.R)")]
        currentFolder: settings.lastFolder ? settings.lastFolder : StandardPaths.writableLocation(StandardPaths.DocumentsLocation)
        onAccepted: win.exportText(selectedFile, "export.r", "text")
    }

    ImportDialog { id: importDialog }

    // Development aid: with SDOE_SCREENSHOT_DIR set, loads the reactor example,
    // steps through a scripted tour saving a picture after each step, and quits.
    Timer {
        id: shots
        property string dir: Backend.screenshotDir()
        property int step: 0
        property var tour: [
            ["setup", () => { Api.call("project.example", { name: "reactor" }); tabs.currentIndex = 0 }],
            ["design", () => tabs.currentIndex = 1],
            ["design_ccd", () => { designPage.select(designPage.methods.find(m => m.id === "ccd")); designPage.runPreview() }],
            ["runs", () => tabs.currentIndex = 2],
            ["edit_text", () => runsPage.startEdit(2, 9, "88.")],
            ["edit_choice", () => { runsPage.startEdit(-1, -1, null); runsPage.startEdit(4, 6, null) }],
            ["analysis", () => tabs.currentIndex = 3],
            ["effects", () => analysisPage.showTab(1)],
            ["profiler", () => analysisPage.showTab(2)],
            ["contour", () => analysisPage.showTab(3)],
            ["surface", () => analysisPage.showSurface3d()],
            ["interaction", () => analysisPage.showTab(4)],
            ["optimize", () => { tabs.currentIndex = 4; optimizePage.optimize() }],
            ["law", () => optimizePage.computeLaw()],
            ["evaluate", () => tabs.currentIndex = 5]
        ]
        running: dir !== ""
        interval: 1500
        repeat: true
        onTriggered: {
            if (step > 0) {
                const name = tour[step - 1][0]
                const k = step
                stop()
                pages.grabToImage(r => {
                    r.saveToFile(dir + "/" + (k < 10 ? "0" : "") + k + "-" + name + ".png")
                    if (k >= tour.length) { quitTimer.start(); return }
                    tour[k][1]()
                    step = k + 1
                    start()
                })
                return
            }
            tour[0][1]()
            step = 1
        }
    }
    Timer { id: quitTimer; interval: 800; onTriggered: Qt.quit() }
}
