//! Runs commands from standard input on one engine and prints each reply on
//! its own line. Each input line is a JSON array `[method, params]`, with
//! `params` an object or null. `js/test.mjs` compares these replies with the
//! WebAssembly build.

use std::io::{BufRead, Write};

fn main() {
    let mut engine = sdoe::Engine::new();
    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    for line in std::io::stdin().lock().lines() {
        let line = line.expect("stdin");
        if line.trim().is_empty() {
            continue;
        }
        let cmd: serde_json::Value = serde_json::from_str(&line).expect("command line");
        let method = cmd[0].as_str().expect("method");
        let params = if cmd[1].is_null() { String::new() } else { cmd[1].to_string() };
        writeln!(out, "{}", engine.call(method, &params)).expect("stdout");
    }
}
