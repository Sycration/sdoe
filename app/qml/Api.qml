pragma Singleton
import QtQuick
import Sdoe

// Command helpers over the Backend singleton.
QtObject {
    id: api

    signal error(string message)
    signal message(string text)

    property var _pending: ({})

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

    // Runs a command on a background thread; `done(result)` runs on success,
    // `failed(error)` (optional) on failure.
    function run(method, params, done, failed) {
        const t = Backend.callAsync(method, JSON.stringify(params || {}))
        _pending[t] = { done: done, failed: failed }
    }

    property Connections _conn: Connections {
        target: Backend
        function onAsyncDone(token, result) {
            const cb = api._pending[token]
            delete api._pending[token]
            const r = JSON.parse(result)
            if (!r.ok) {
                if (cb && cb.failed) cb.failed(r.error)
                else api.error(r.error)
            } else if (cb && cb.done) {
                cb.done(r.result)
            }
        }
    }
}
