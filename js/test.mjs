// Tests the JavaScript interface and compares the WebAssembly engine with
// the native one.
//
//   node js/test.mjs <sdoe.wasm> [replay command...]
//
// The replay command (e.g. `cargo run --release -p sdoe-core --example
// replay`) runs the same commands natively; without it, only the
// WebAssembly side runs.

import { init, initSync, version, Engine } from "./sdoe.js";
import { readFileSync, readdirSync } from "node:fs";
import { spawnSync } from "node:child_process";
import { pathToFileURL, fileURLToPath } from "node:url";
import assert from "node:assert/strict";

const [wasmPath, ...replay] = process.argv.slice(2);
if (!wasmPath) {
    console.error("usage: node js/test.mjs <sdoe.wasm> [replay command...]");
    process.exit(2);
}
const root = fileURLToPath(new URL("..", import.meta.url));
const pkg = JSON.parse(readFileSync(new URL("package.json", import.meta.url)));

assert.throws(() => new Engine(), /init\(\) first/);
const loading = init(pathToFileURL(wasmPath));
assert.equal(init(), loading);
await loading;
assert.equal(version(), pkg.version);

// Interface behaviour.
{
    const e = new Engine();
    assert.deepEqual(e.call("no.such"), { ok: false, error: "unknown method 'no.such'" });
    assert.equal(e.call("project.state", "[1]").ok, false);
    assert.match(e.call("project.state", "{").error, /^invalid JSON/);
    const r = e.call("project.example", { name: "reactor" });
    assert.equal(r.ok, true);
    assert.equal(r.changed, true);
    assert.equal(typeof e.callJson("project.state"), "string");
    const state = e.call("project.state").result;
    assert.ok(state.runs > 0 && state.factors > 0);
    assert.equal(e.call("project.set", { title: "Ünïcode ✓ \0" }).ok, true);
    assert.equal(e.call("project.state").result.title, "Ünïcode ✓ \0");
    const big = "x".repeat(1 << 20);
    assert.equal(e.call("project.set", { notes: big }).ok, true);
    assert.equal(e.call("project.state").result.notes.length, big.length);
    // A second engine has its own project.
    const f = new Engine();
    assert.equal(f.call("project.state").result.runs, 0);
    f.free();
    f.free();
    assert.throws(() => f.call("project.state"), /freed/);
    e[Symbol.dispose]();
}
// Synchronous loading.
initSync(readFileSync(wasmPath));
assert.equal(new Engine().call("app.version").result, pkg.version);

// Command sequence over every example project and both built-in examples.
const commands = [];
const replies = [];
const engine = new Engine();
const run = (method, params = null) => {
    const reply = engine.callJson(method, params === null ? "" : JSON.stringify(params));
    commands.push(JSON.stringify([method, params]));
    replies.push(reply);
    return JSON.parse(reply);
};

const projects = readdirSync(`${root}/examples`).filter((f) => f.endsWith(".sdoe")).sort();
const sources = [["project.example", { name: "reactor" }], ["project.example", { name: "blend" }],
    ...projects.map((f) => ["project.open", { text: readFileSync(`${root}/examples/${f}`, "utf8") }])];
for (const [method, params] of sources) {
    assert.equal(run(method, params).ok, true, `${method} ${params.name ?? ""}`);
    run("project.state");
    run("factors.list");
    const responses = run("responses.list").result;
    for (const r of responses) {
        run("analysis.fit", { response: r.id });
        run("analysis.equation", { response: r.id, units: "actual", syntax: "plain" });
        run("analysis.equation", { response: r.id, units: "coded", syntax: "python" });
    }
    run("optimize.run", {});
    run("export.csv");
    run("export.report");
    run("export.r");
    const methods = run("design.methods").result;
    const first = (methods.methods ?? methods)[0];
    if (first?.key ?? first?.id) run("design.preview", { method: first.key ?? first.id, options: {} });
    if (responses.length) run("model.auto", { response: responses[0].id });
    run("project.save");
}
const failed = replies.filter((r) => !JSON.parse(r).ok).length;
console.log(`wasm: ${commands.length} commands over ${sources.length} projects (${failed} error replies)`);

if (replay.length) {
    const native = spawnSync(replay[0], replay.slice(1), { input: commands.join("\n") + "\n", encoding: "utf8", maxBuffer: 1 << 30, stdio: ["pipe", "pipe", "inherit"] });
    assert.equal(native.status, 0, "replay failed");
    const lines = native.stdout.split("\n").slice(0, -1);
    assert.equal(lines.length, replies.length);
    // Rust's libm in WebAssembly and the system libm round some functions
    // differently in the last bit, so numbers (also inside text) are
    // compared with a relative tolerance.
    let differ = 0;
    let worst = 0;
    const number = /-?\d+(?:\.\d+)?(?:[eE][-+]?\d+)?/g;
    const close = (a, b) => {
        const d = Math.abs(a - b) / Math.max(Math.abs(a), Math.abs(b), 1e-300);
        if (a !== b) worst = Math.max(worst, d);
        return a === b || d <= 1e-9;
    };
    const same = (a, b) => {
        if (typeof a === "number" && typeof b === "number") return close(a, b);
        if (typeof a === "string" && typeof b === "string") {
            if (a === b) return true;
            const na = a.match(number) ?? [], nb = b.match(number) ?? [];
            // Number lengths move the line breaks of wrapped text.
            const text = (t) => t.split(number).join("#").replace(/\s+/g, "");
            return text(a) === text(b) && na.length === nb.length && na.every((x, i) => close(Number(x), Number(nb[i])));
        }
        if (Array.isArray(a) && Array.isArray(b)) return a.length === b.length && a.every((x, i) => same(x, b[i]));
        if (a && b && typeof a === "object" && typeof b === "object") {
            const ka = Object.keys(a), kb = Object.keys(b);
            return ka.length === kb.length && ka.every((k) => k in b && same(a[k], b[k]));
        }
        return a === b;
    };
    let exact = 0;
    for (let i = 0; i < lines.length; i++) {
        if (lines[i] === replies[i]) {
            exact += 1;
        } else if (!same(JSON.parse(lines[i]), JSON.parse(replies[i]))) {
            differ += 1;
            if (differ <= 5) console.log(`differs: ${commands[i].slice(0, 120)}\n  native ${lines[i].slice(0, 300)}\n  wasm   ${replies[i].slice(0, 300)}`);
        }
    }
    assert.equal(differ, 0, `${differ} replies differ from the native engine`);
    console.log(`native: ${exact} replies identical, ${lines.length - exact} within rounding (largest relative difference ${worst.toExponential(1)})`);
}
console.log("ok");
