//! Helpers shared by the regression test files.
#![allow(dead_code)]

use sdoe::Engine;
use serde_json::{Value, json};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// Raw reply of one engine command.
pub fn reply(e: &mut Engine, m: &str, p: Value) -> Value {
    serde_json::from_str(&e.call(m, &p.to_string())).unwrap()
}

/// Result of a command that must succeed.
pub fn call(e: &mut Engine, m: &str, p: Value) -> Value {
    let out = reply(e, m, p);
    assert_eq!(out["ok"], json!(true), "{m} failed: {out}");
    out["result"].clone()
}

/// Error text of a command that must fail.
pub fn call_err(e: &mut Engine, m: &str, p: Value) -> String {
    let out = reply(e, m, p);
    assert_eq!(out["ok"], json!(false), "{m} unexpectedly succeeded: {out}");
    out["error"].as_str().unwrap().to_string()
}

/// Text of a project file in the repository's `examples/` directory.
pub fn example_text(file: &str) -> String {
    let path = format!("{}/../examples/{file}", env!("CARGO_MANIFEST_DIR"));
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"))
}

/// Engine with a project file from `examples/` loaded.
pub fn engine_with_example(file: &str) -> Engine {
    engine_with_text(&example_text(file))
}

/// Engine with the given project JSON loaded.
pub fn engine_with_project(p: &Value) -> Engine {
    engine_with_text(&p.to_string())
}

pub fn engine_with_text(text: &str) -> Engine {
    let mut e = Engine::new();
    call(&mut e, "project.open", json!({ "text": text }));
    e
}

/// Factor JSON in the project file format. `id` 1 gets symbol A, 2 gets B, ….
pub fn cont(id: u64, low: f64, high: f64) -> Value {
    factor(id, json!({"type": "continuous", "low": low, "high": high}))
}
pub fn mix(id: u64, low: f64, high: f64) -> Value {
    factor(id, json!({"type": "mixture", "low": low, "high": high}))
}
pub fn cat(id: u64, levels: &[&str]) -> Value {
    factor(id, json!({"type": "categorical", "levels": levels}))
}
pub fn disc(id: u64, levels: &[f64]) -> Value {
    factor(id, json!({"type": "discrete", "levels": levels}))
}
pub fn factor(id: u64, kind: Value) -> Value {
    let sym = ((b'A' + (id as u8 - 1)) as char).to_string();
    json!({"id": id, "symbol": sym, "name": format!("F{id}"), "units": "", "kind": kind, "role": "controlled"})
}

/// Project JSON with the given factors and no runs or responses.
pub fn project(factors: Vec<Value>) -> Value {
    json!({
        "format": 1, "title": "t", "notes": "", "factors": factors, "responses": [], "runs": [],
        "models": {}, "mixture_total": 1.0, "next_id": 1000, "optimize": {"env": {}, "fixed": {}}
    })
}

/// Outcome of a test body run in a child process.
#[derive(Debug)]
pub enum Isolated {
    Passed,
    Failed { status: String, stderr: String },
    TimedOut,
}

/// Runs the test `name` of the current test binary in a child process with a
/// time limit and (on Unix) an address-space limit, and panics unless the
/// child passes. Use it through [`isolated_test!`] for bodies that may hang,
/// abort, overflow the stack or exhaust memory.
pub fn run_isolated(name: &str, timeout_secs: u64, mem_mb: u64) {
    match isolated_outcome(name, timeout_secs, mem_mb) {
        Isolated::Passed => {}
        Isolated::TimedOut => panic!("{name}: no result within {timeout_secs} s"),
        Isolated::Failed { status, stderr } => {
            let tail: String = stderr.lines().rev().take(15).collect::<Vec<_>>().into_iter().rev().collect::<Vec<_>>().join("\n");
            panic!("{name}: child process failed ({status}):\n{tail}")
        }
    }
}

pub fn isolated_outcome(name: &str, timeout_secs: u64, mem_mb: u64) -> Isolated {
    let exe = std::env::current_exe().unwrap();
    let args = [name, "--exact", "--nocapture", "--test-threads=1"];
    #[cfg(unix)]
    let mut cmd = {
        let mut c = Command::new("sh");
        c.arg("-c").arg(format!("ulimit -v {} 2>/dev/null; exec \"$0\" \"$@\"", mem_mb * 1024)).arg(&exe).args(args);
        c
    };
    #[cfg(not(unix))]
    let mut cmd = {
        let _ = mem_mb;
        let mut c = Command::new(&exe);
        c.args(args);
        c
    };
    cmd.env("SDOE_ISOLATED", name).stdout(Stdio::null()).stderr(Stdio::piped());
    let mut child = cmd.spawn().unwrap();
    let start = Instant::now();
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            let mut stderr = String::new();
            if let Some(mut s) = child.stderr.take() {
                use std::io::Read;
                let _ = s.read_to_string(&mut stderr);
            }
            return if status.success() { Isolated::Passed } else { Isolated::Failed { status: status.to_string(), stderr } };
        }
        if start.elapsed() > Duration::from_secs(timeout_secs) {
            let _ = child.kill();
            let _ = child.wait();
            return Isolated::TimedOut;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// True in the child process that runs the body of the isolated test `name`.
pub fn in_isolated_child(name: &str) -> bool {
    std::env::var("SDOE_ISOLATED").is_ok_and(|v| v == name)
}

/// Declares a test whose body runs in a child process with a time limit (s)
/// and a memory limit (MB). The test passes only when the body completes
/// without panicking, aborting or running out of time or memory.
///
/// ```ignore
/// isolated_test!(deep_parens, 20, 2048, {
///     let mut e = engine_with_example("x.sdoe");
///     /* … */
/// });
/// ```
#[macro_export]
macro_rules! isolated_test {
    ($(#[$attr:meta])* $name:ident, $secs:expr, $mb:expr, $body:block) => {
        $(#[$attr])*
        #[test]
        fn $name() {
            if $crate::common::in_isolated_child(stringify!($name)) {
                $body
            } else {
                $crate::common::run_isolated(stringify!($name), $secs, $mb);
            }
        }
    };
}
