// sdoe - design-of-experiments engine, JavaScript interface. See API.md.

/** Reply of a command. */
export type Reply<T = any> =
    | { ok: true; result: T; changed: boolean; revision: number }
    | { ok: false; error: string };

export type Source =
    | string
    | URL
    | Response
    | Promise<Response>
    | BufferSource
    | WebAssembly.Module;

/**
 * Loads the WebAssembly module. Without an argument, sdoe.wasm is loaded
 * from the directory of sdoe.js. Later calls return the same promise.
 */
export function init(source?: Source): Promise<void>;

/** Loads the WebAssembly module synchronously from its bytes or a compiled module. */
export function initSync(source: BufferSource | WebAssembly.Module): void;

/** Library version, e.g. "0.1.0". */
export function version(): string;

/** An engine with its own project, undo history and design preview. */
export class Engine {
    /** Creates an engine with an empty project. Requires init(). */
    constructor();
    /** Runs a command. `params` is an object or JSON text; omit it for no parameters. */
    call<T = any>(method: string, params?: object | string | null): Reply<T>;
    /** Runs a command with JSON text parameters and returns the reply as JSON text. */
    callJson(method: string, paramsJson?: string): string;
    /** Destroys the engine. Further calls throw. */
    free(): void;
    [Symbol.dispose](): void;
}
