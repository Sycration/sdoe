pragma Singleton
import QtQuick
import Sdoe

// Command helpers over the Backend singleton.
QtObject {
    id: api

    signal error(string message)
    signal message(string text)
    // Asks open editors to send their value to the engine now (before saving).
    signal commitRequested()

    property var _pending: ({})
    // Per key of `fetchLatest`: whether a command runs, and the request queued behind it.
    property var _latest: ({})
    // The WebAssembly build has no threads: background commands run on the
    // interface thread, so the busy overlay needs a frame before they start.
    readonly property bool _inline: Qt.platform.os === "wasm"
    // Commands of `run` waiting for that frame.
    property int waiting: 0
    readonly property bool busy: Backend.busy || waiting > 0

    // Runs a command; returns the result, or undefined after reporting an error.
    function call(method, params) {
        const r = JSON.parse(Backend.call(method, JSON.stringify(params || {})))
        if (!r.ok) {
            api.error(r.error)
            return undefined
        }
        return r.result
    }

    // Runs a command without reporting errors; returns {ok, result, error}.
    function tryCall(method, params) {
        return JSON.parse(Backend.call(method, JSON.stringify(params || {})))
    }

    // Runs a command on a background thread with the busy overlay; `done(result)`
    // runs on success, `failed(error)` (optional) on failure.
    function run(method, params, done, failed) {
        const start = () => {
            const t = Backend.callAsync(method, JSON.stringify(params || {}))
            _pending[t] = { done: done, failed: failed }
        }
        if (!_inline) { start(); return }
        api.waiting += 1
        const timer = Qt.createQmlObject("import QtQuick; Timer { interval: 60 }", api)
        timer.triggered.connect(() => {
            api.waiting -= 1
            timer.destroy()
            start()
        })
        timer.start()
    }

    // Runs a command on a background thread without the busy overlay;
    // `reply({ok, result, error})` receives the reply.
    function fetch(method, params, reply) {
        const t = Backend.callBackground(method, JSON.stringify(params || {}))
        _pending[t] = { reply: reply }
    }

    // Like `fetch`, with at most one command per `key` at a time: a request
    // made while one runs replaces any queued request, and the running one's
    // reply is dropped once a newer request waits. Refreshes driven by
    // dragging or typing so stay current without piling up work.
    function fetchLatest(key, method, params, reply) {
        let s = _latest[key]
        if (s && s.busy) {
            s.next = { method: method, params: params, reply: reply }
            return
        }
        s = { busy: true, next: null }
        _latest[key] = s
        fetch(method, params, r => {
            const n = s.next
            s.busy = false
            s.next = null
            if (n) fetchLatest(key, n.method, n.params, n.reply)
            else reply(r)
        })
    }

    property Connections _conn: Connections {
        target: Backend
        function onAsyncDone(token, result) {
            const cb = api._pending[token]
            delete api._pending[token]
            const r = JSON.parse(result)
            if (cb && cb.reply) {
                cb.reply(r)
            } else if (!r.ok) {
                if (cb && cb.failed) cb.failed(r.error)
                else api.error(r.error)
            } else if (cb && cb.done) {
                cb.done(r.result)
            }
        }
    }
}
