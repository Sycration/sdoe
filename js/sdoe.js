// sdoe - design-of-experiments engine, JavaScript interface.
//
// The engine runs as WebAssembly (sdoe.wasm, next to this file). Each command
// takes a method name and a parameter object and returns a reply object:
//
//   {ok: true, result: <value>, changed: <bool>, revision: <n>}
//   {ok: false, error: "<message>"}
//
// The reply format, the methods and their parameters are the same as in the
// C interface; see API.md.
//
//   import { init, Engine } from "./sdoe.js";
//   await init();
//   const engine = new Engine();
//   engine.call("project.example", { name: "reactor" });
//   const fit = engine.call("analysis.fit", { response: "6" });
//   engine.free();

let wasm = null;
let generation = 0;
let pending = null;

const encoder = new TextEncoder();
const decoder = new TextDecoder();

const LOST = { ok: false, error: "the engine was lost after an internal error; call init() and create a new Engine" };

function instantiated(instance) {
    wasm = instance.exports;
    generation += 1;
}

async function bytesOf(source) {
    if (source instanceof URL && source.protocol === "file:") {
        const fs = await import("node:fs/promises");
        return fs.readFile(source);
    }
    const response = await fetch(source);
    if (!response.ok) throw new Error(`sdoe: cannot load ${source}: HTTP ${response.status}`);
    return response;
}

async function compile(source) {
    if (source instanceof WebAssembly.Module) return WebAssembly.instantiate(source, {});
    if (source === undefined) source = new URL("./sdoe.wasm", import.meta.url);
    if (typeof source === "string") source = new URL(source, import.meta.url);
    if (source instanceof URL) source = await bytesOf(source);
    source = await source;
    if (typeof Response !== "undefined" && source instanceof Response) {
        if (WebAssembly.instantiateStreaming && source.headers.get("content-type") === "application/wasm") {
            return (await WebAssembly.instantiateStreaming(source, {})).instance;
        }
        source = await source.arrayBuffer();
    }
    return (await WebAssembly.instantiate(source, {})).instance;
}

/**
 * Loads the WebAssembly module. Without an argument, sdoe.wasm is loaded
 * from the directory of this file. `source` may be a URL or path string, a
 * URL, a Response (or a promise of one), the module bytes, or a compiled
 * WebAssembly.Module. Later calls return the same promise, unless a previous
 * instance was lost after an internal error.
 */
export function init(source) {
    if (pending) return pending;
    pending = compile(source).then(instantiated, (e) => {
        pending = null;
        throw e;
    });
    return pending;
}

/**
 * Loads the WebAssembly module synchronously from its bytes or a compiled
 * WebAssembly.Module. Browsers limit synchronous compilation of large
 * modules to workers.
 */
export function initSync(source) {
    const module = source instanceof WebAssembly.Module ? source : new WebAssembly.Module(source);
    instantiated(new WebAssembly.Instance(module, {}));
    pending = Promise.resolve();
}

function loaded() {
    if (!wasm) throw new Error("sdoe: call init() first");
    return wasm;
}

// Copies a string into WebAssembly memory as NUL-terminated UTF-8.
function put(w, s) {
    const bytes = encoder.encode(s.replaceAll("\0", ""));
    const ptr = w.sdoe_alloc(bytes.length + 1);
    const mem = new Uint8Array(w.memory.buffer, ptr, bytes.length + 1);
    mem.set(bytes);
    mem[bytes.length] = 0;
    return [ptr, bytes.length + 1];
}

// Reads and frees a NUL-terminated string returned by the library.
function take(w, ptr) {
    if (ptr === 0) return null;
    const mem = new Uint8Array(w.memory.buffer);
    let end = ptr;
    while (mem[end] !== 0) end += 1;
    const s = decoder.decode(mem.subarray(ptr, end));
    w.sdoe_string_free(ptr);
    return s;
}

/** Library version, e.g. "0.1.0". */
export function version() {
    const w = loaded();
    return take(w, w.sdoe_version());
}

const registry = typeof FinalizationRegistry === "undefined" ? null : new FinalizationRegistry(({ w, gen, handle }) => {
    if (gen === generation) w.sdoe_engine_free(handle);
});

/** An engine with its own project, undo history and design preview. */
export class Engine {
    #w;
    #gen;
    #handle;

    /** Creates an engine with an empty project. Requires init(). */
    constructor() {
        this.#w = loaded();
        this.#gen = generation;
        this.#handle = this.#w.sdoe_engine_new();
        registry?.register(this, { w: this.#w, gen: this.#gen, handle: this.#handle }, this);
    }

    /**
     * Runs a command and returns the reply as JSON text. `paramsJson` may
     * be omitted or "" for no parameters.
     */
    callJson(method, paramsJson = "") {
        if (this.#handle === 0) throw new Error("sdoe: the engine was freed");
        if (this.#gen !== generation) return JSON.stringify(LOST);
        const w = this.#w;
        const [m, mlen] = put(w, String(method));
        const [p, plen] = put(w, String(paramsJson ?? ""));
        let out;
        try {
            out = w.sdoe_call(this.#handle, m, p);
        } catch (e) {
            if (!(e instanceof WebAssembly.RuntimeError)) throw e;
            // A Rust panic stops the WebAssembly instance in an unknown
            // state, so the instance and all its engines are dropped.
            wasm = null;
            pending = null;
            generation += 1;
            return JSON.stringify({ ok: false, error: `internal error (${e.message})` });
        }
        w.sdoe_dealloc(m, mlen);
        w.sdoe_dealloc(p, plen);
        return take(w, out);
    }

    /**
     * Runs a command. `params` is an object (or JSON text) and may be
     * omitted for no parameters. Returns the parsed reply.
     */
    call(method, params) {
        const text = params === undefined || params === null ? "" : typeof params === "string" ? params : JSON.stringify(params);
        return JSON.parse(this.callJson(method, text));
    }

    /** Destroys the engine. Further calls throw. */
    free() {
        if (this.#handle === 0) return;
        registry?.unregister(this);
        if (this.#gen === generation) this.#w.sdoe_engine_free(this.#handle);
        this.#handle = 0;
    }

    [Symbol.dispose ?? Symbol.for("Symbol.dispose")]() {
        this.free();
    }
}
