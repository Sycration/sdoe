import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Sdoe

// Viewer of the user documentation compiled into the program, with a search
// over all pages. The search finds the sections that contain every word of
// the query and marks the words on the displayed page.
ApplicationWindow {
    id: help
    width: 1040
    height: 760
    minimumWidth: 600
    minimumHeight: 420
    title: qsTr("sdoe documentation")
    color: Theme.window

    // [{file, title, text, headings: [{level, text, slug}]}]
    property var pages: []
    // Per page: displayed plain text, its lower-case form, and the position
    // of each heading (same order as `headings`, -1 when not found).
    property var plain: []
    property var lower: []
    property var headingPos: []
    property int current: -1
    // Visited places [{page, pos}] for Back and Forward.
    property var history: []
    property int historyIndex: -1
    property var terms: []
    property var results: []
    // Range of the result being shown; marked more strongly than the others.
    property int activeStart: -1
    property int activeEnd: -1
    // Rectangles of the marked words: [{x, y, w, h, active}].
    property var marks: []

    function load() {
        if (pages.length) return
        const ps = JSON.parse(Backend.helpPages())
        const pl = [], lo = [], hp = []
        for (const p of ps) {
            probe.text = p.text
            // Paragraph, line and table-cell separators become line breaks,
            // one character each, so positions stay those of the document.
            const t = probe.getText(0, probe.length).replace(/[\u2028\u2029\ufdd0\ufdd1]/g, "\n")
            pl.push(t)
            lo.push(t.toLowerCase())
            hp.push(headingPositions(t, p.headings))
        }
        probe.text = ""
        plain = pl
        lower = lo
        headingPos = hp
        pages = ps
    }

    // Start of the line of each heading, searched in order through the text.
    function headingPositions(text, headings) {
        const padded = "\n" + text + "\n"
        const out = []
        let from = 0
        for (const h of headings) {
            const p = padded.indexOf("\n" + h.text + "\n", from)
            out.push(p)
            if (p >= 0) from = p + 1
        }
        return out
    }

    // Opens the window at the contents, or at the page of `file`.
    function openAt(file) {
        load()
        const i = Math.max(0, pages.findIndex(p => p.file === file))
        if (current < 0 || file) navigate(i, 0, -1, -1)
        if (Backend.webFiles()) showMaximized()
        else show()
        raise()
        requestActivate()
        search.forceActiveFocus()
    }

    // Shows page `page` at text position `pos`; `start` and `end` give the
    // range of a search result (-1 for none). Adds the place to the history.
    function navigate(page, pos, start, end) {
        const h = history.slice(0, historyIndex + 1)
        if (historyIndex >= 0) h[historyIndex] = { page: current, pos: viewPos() }
        h.push({ page: page, pos: pos })
        history = h
        historyIndex = h.length - 1
        display(page, pos, start, end)
    }

    function goBack() {
        if (historyIndex <= 0) return
        const h = history.slice()
        h[historyIndex] = { page: current, pos: viewPos() }
        history = h
        historyIndex -= 1
        display(h[historyIndex].page, h[historyIndex].pos, -1, -1)
    }

    function goForward() {
        if (historyIndex >= history.length - 1) return
        historyIndex += 1
        display(history[historyIndex].page, history[historyIndex].pos, -1, -1)
    }

    // Text position at the top of the visible part of the page.
    function viewPos() {
        return current < 0 ? 0 : view.positionAt(0, Math.max(0, flick.contentY - view.y))
    }

    function display(page, pos, start, end) {
        if (page !== current) {
            current = page
            view.text = pages[page].text
        }
        activeStart = start
        activeEnd = end
        Qt.callLater(() => {
            const r = view.positionToRectangle(pos)
            const y = pos <= 0 ? 0 : view.y + r.y - (start >= 0 ? flick.height / 3 : 8)
            const bottom = 2 * view.y + view.implicitHeight - flick.height
            flick.contentY = Math.max(0, Math.min(y, bottom))
            updateMarks()
        })
    }

    // Follows a link of the displayed page.
    function follow(link) {
        if (link.indexOf("://") >= 0) { Qt.openUrlExternally(link); return }
        const hash = link.indexOf("#")
        const file = hash >= 0 ? link.substring(0, hash) : link
        const anchor = hash >= 0 ? link.substring(hash + 1) : ""
        const page = file ? pages.findIndex(p => p.file === file) : current
        if (page < 0) return
        let pos = 0
        if (anchor) {
            const k = pages[page].headings.findIndex(h => h.slug === anchor)
            if (k >= 0 && headingPos[page][k] >= 0) pos = headingPos[page][k]
        }
        navigate(page, pos, -1, -1)
    }

    // Position of `term` at or after `from` where it starts a word, or -1.
    function findWord(text, term, from) {
        let p = text.indexOf(term, from)
        while (p > 0 && /[\p{L}\p{N}]/u.test(text.charAt(p - 1)))
            p = text.indexOf(term, p + 1)
        return p
    }

    // Heading of the section that contains `pos`, or -1.
    function sectionOf(page, pos) {
        let best = -1
        const hp = headingPos[page]
        for (let k = 0; k < hp.length; ++k)
            if (hp[k] >= 0 && hp[k] <= pos && (best < 0 || hp[k] >= hp[best])) best = k
        return best
    }

    function runSearch() {
        const t = search.text.toLowerCase().split(/\s+/).filter(s => s.length > 0)
        terms = t
        if (!t.length) { results = []; updateMarks(); return }
        const out = []
        for (let i = 0; i < pages.length; ++i) {
            const text = lower[i]
            // Section starts: the page start and every heading found.
            const starts = [0].concat(headingPos[i].filter(p => p > 0)).sort((a, b) => a - b)
            for (let s = 0; s < starts.length; ++s) {
                const a = starts[s]
                const b = s + 1 < starts.length ? starts[s + 1] : text.length
                const seg = text.substring(a, b)
                let first = -1, firstLen = 0, count = 0, inHeading = 0
                const headEnd = seg.indexOf("\n")
                let all = true
                for (const term of t) {
                    let p = findWord(seg, term, 0)
                    if (p < 0) { all = false; break }
                    if (headEnd < 0 || p < headEnd) inHeading += 1
                    if (first < 0 || p < first) { first = p; firstLen = term.length }
                    while (p >= 0) { count += 1; p = findWord(seg, term, p + term.length) }
                }
                if (!all) continue
                const pos = a + first
                const k = sectionOf(i, pos)
                const from = Math.max(a, pos - 50)
                const snippet = (from > a ? "…" : "") + plain[i].substring(from, Math.min(b, pos + 120)).replace(/\s+/g, " ").trim() + "…"
                out.push({
                    page: i, pos: pos, end: pos + firstLen, order: out.length,
                    score: count + 10 * inHeading,
                    place: pages[i].title + (k > 0 ? " › " + pages[i].headings[k].text : ""),
                    snippet: snippet
                })
            }
        }
        out.sort((x, y) => y.score - x.score || x.order - y.order)
        results = out.slice(0, 200)
        updateMarks()
    }

    // Searches for `query` and shows the first result.
    function searchFor(query) {
        search.text = query
        openFirstResult()
    }

    function openFirstResult() {
        searchDelay.stop()
        runSearch()
        if (!results.length) return
        resultList.currentIndex = 0
        openResult(results[0])
    }

    function openResult(r) {
        navigate(r.page, r.pos, r.pos, r.end)
    }

    // Marks every occurrence of the search words on the displayed page.
    function updateMarks() {
        if (current < 0 || !terms.length) { marks = []; return }
        const text = lower[current]
        const out = []
        for (const term of terms) {
            let p = findWord(text, term, 0)
            while (p >= 0 && out.length < 1000) {
                const r1 = view.positionToRectangle(p)
                const r2 = view.positionToRectangle(p + term.length)
                const w = r2.y === r1.y ? r2.x - r1.x : view.width - r1.x
                out.push({ x: r1.x, y: r1.y, w: Math.max(2, w), h: r1.height, active: p === activeStart })
                p = findWord(text, term, p + term.length)
            }
        }
        marks = out
    }

    Timer { id: searchDelay; interval: 150; onTriggered: help.runSearch() }

    Shortcut { sequences: [StandardKey.Back]; onActivated: help.goBack() }
    Shortcut { sequences: [StandardKey.Forward]; onActivated: help.goForward() }
    Shortcut { sequences: [StandardKey.Find]; onActivated: { search.forceActiveFocus(); search.selectAll() } }
    Shortcut { sequences: [StandardKey.Close]; onActivated: help.close() }

    // Renders a page once to obtain its displayed text.
    TextEdit { id: probe; visible: false; textFormat: TextEdit.MarkdownText }

    // The whole content, as one item (the screenshot tour grabs it).
    property alias frame: frame

    Page {
        id: frame
        anchors.fill: parent
        background: Rectangle { color: Theme.window }

        header: ToolBar {
            RowLayout {
                anchors.fill: parent
                anchors.leftMargin: 6
                anchors.rightMargin: 6
                ToolButton {
                    text: "←"
                    enabled: help.historyIndex > 0
                    onClicked: help.goBack()
                    ToolTip.visible: hovered
                    ToolTip.text: qsTr("Back")
                }
                ToolButton {
                    text: "→"
                    enabled: help.historyIndex < help.history.length - 1
                    onClicked: help.goForward()
                    ToolTip.visible: hovered
                    ToolTip.text: qsTr("Forward")
                }
                TextField {
                    id: search
                    Layout.preferredWidth: 300
                    placeholderText: qsTr("Search")
                    selectByMouse: true
                    onTextChanged: searchDelay.restart()
                    Keys.onReturnPressed: help.openFirstResult()
                    Keys.onEnterPressed: help.openFirstResult()
                    Keys.onDownPressed: if (help.results.length) { resultList.forceActiveFocus(); resultList.currentIndex = 0 }
                    Keys.onEscapePressed: if (text) text = ""; else help.close()
                }
                Item { Layout.fillWidth: true }
            }
        }

        SplitView {
            anchors.fill: parent

            Rectangle {
                SplitView.preferredWidth: 280
                SplitView.minimumWidth: 180
                color: Theme.base

                // Contents: the pages, with the sections of the displayed page.
                ListView {
                    id: contents
                    anchors.fill: parent
                    visible: help.terms.length === 0
                    clip: true
                    ScrollBar.vertical: ScrollBar {}
                    model: {
                        const out = []
                        for (let i = 0; i < help.pages.length; ++i) {
                            out.push({ page: i, heading: -1, text: help.pages[i].title, level: 0 })
                            if (i !== help.current) continue
                            const hs = help.pages[i].headings
                            for (let k = 0; k < hs.length; ++k)
                                if (hs[k].level === 2 && help.headingPos[i][k] >= 0)
                                    out.push({ page: i, heading: k, text: hs[k].text, level: 1 })
                        }
                        return out
                    }
                    delegate: ItemDelegate {
                        required property var modelData
                        width: ListView.view.width
                        leftPadding: 12 + modelData.level * 16
                        text: modelData.text
                        font.bold: modelData.level === 0 && modelData.page === help.current
                        highlighted: modelData.level === 0 && modelData.page === help.current
                        onClicked: help.navigate(modelData.page, modelData.heading < 0 ? 0 : help.headingPos[modelData.page][modelData.heading], -1, -1)
                    }
                }

                ListView {
                    id: resultList
                    anchors.fill: parent
                    visible: help.terms.length > 0
                    clip: true
                    model: help.results
                    currentIndex: -1
                    ScrollBar.vertical: ScrollBar {}
                    Keys.onReturnPressed: if (currentIndex >= 0) help.openResult(help.results[currentIndex])
                    Keys.onEnterPressed: if (currentIndex >= 0) help.openResult(help.results[currentIndex])
                    Keys.onUpPressed: { if (currentIndex <= 0) search.forceActiveFocus(); else decrementCurrentIndex() }
                    onCurrentIndexChanged: if (activeFocus && currentIndex >= 0) help.openResult(help.results[currentIndex])
                    delegate: ItemDelegate {
                        id: hit
                        required property var modelData
                        required property int index
                        width: ListView.view.width
                        highlighted: ListView.isCurrentItem
                        onClicked: { resultList.currentIndex = index; help.openResult(modelData) }
                        contentItem: ColumnLayout {
                            spacing: 2
                            Label {
                                Layout.fillWidth: true
                                text: hit.modelData.place
                                font.bold: true
                                elide: Text.ElideRight
                                color: hit.highlighted ? Theme.highlightedText : Theme.text
                            }
                            Label {
                                Layout.fillWidth: true
                                text: hit.modelData.snippet
                                wrapMode: Text.Wrap
                                maximumLineCount: 3
                                elide: Text.ElideRight
                                color: hit.highlighted ? Theme.highlightedText : Theme.muted
                            }
                        }
                    }
                    Label {
                        anchors.centerIn: parent
                        visible: help.results.length === 0
                        color: Theme.muted
                        text: qsTr("No matches")
                    }
                }
            }

            Rectangle {
                SplitView.fillWidth: true
                color: Theme.base

                Flickable {
                    id: flick
                    anchors.fill: parent
                    clip: true
                    contentWidth: width
                    contentHeight: pageItem.height
                    boundsBehavior: Flickable.StopAtBounds
                    ScrollBar.vertical: ScrollBar {}
                    onWidthChanged: Qt.callLater(help.updateMarks)

                    Item {
                        id: pageItem
                        width: flick.width
                        height: view.height + 2 * view.y

                        Repeater {
                            model: help.marks
                            delegate: Rectangle {
                                required property var modelData
                                x: view.x + modelData.x
                                y: view.y + modelData.y
                                width: modelData.w
                                height: modelData.h
                                radius: 2
                                color: Qt.rgba(Theme.highlight.r, Theme.highlight.g, Theme.highlight.b, modelData.active ? 0.6 : 0.25)
                            }
                        }

                        TextEdit {
                            id: view
                            x: 28
                            y: 18
                            width: Math.min(pageItem.width - 2 * x, 920)
                            readOnly: true
                            selectByMouse: true
                            persistentSelection: true
                            textFormat: TextEdit.MarkdownText
                            wrapMode: TextEdit.Wrap
                            color: Theme.text
                            selectionColor: Theme.highlight
                            selectedTextColor: Theme.highlightedText
                            onLinkActivated: link => help.follow(link)
                            HoverHandler { cursorShape: view.hoveredLink ? Qt.PointingHandCursor : Qt.IBeamCursor }
                        }
                    }
                }
            }
        }
    }
}
