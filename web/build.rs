//! Embeds the WebAssembly build of the application: every file in the
//! directory named by SDOE_WEB_APP (default: target/web-app), gzip-compressed,
//! plus web/static. Without a build, the server delivers a page that says so.

use flate2::{Compression, write::GzEncoder};
use std::io::Write;
use std::path::{Path, PathBuf};

fn mime(name: &str) -> &'static str {
    match name.rsplit('.').next().unwrap_or("") {
        "html" => "text/html; charset=utf-8",
        "js" => "text/javascript; charset=utf-8",
        "wasm" => "application/wasm",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "ico" => "image/x-icon",
        "css" => "text/css; charset=utf-8",
        "json" => "application/json",
        _ => "application/octet-stream",
    }
}

// FNV-1a, for the ETag.
fn fnv(data: &[u8]) -> u64 {
    data.iter().fold(0xcbf29ce484222325u64, |h, &b| (h ^ b as u64).wrapping_mul(0x100000001b3))
}

fn files(dir: &Path) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = match std::fs::read_dir(dir) {
        Ok(rd) => rd.filter_map(|e| e.ok()).map(|e| e.path()).filter(|p| p.is_file()).collect(),
        Err(_) => Vec::new(),
    };
    v.sort();
    v
}

fn main() {
    let manifest = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let out = PathBuf::from(std::env::var("OUT_DIR").unwrap());
    println!("cargo:rerun-if-env-changed=SDOE_WEB_APP");
    let app = std::env::var_os("SDOE_WEB_APP").map(PathBuf::from).unwrap_or_else(|| manifest.join("../target/web-app"));
    let statics = manifest.join("static");
    println!("cargo:rerun-if-changed={}", app.display());
    println!("cargo:rerun-if-changed={}", statics.display());

    let app_files = files(&app);
    let built = app_files.iter().any(|p| p.file_name().is_some_and(|n| n == "sdoe.wasm"));
    if !built {
        println!("cargo:warning=no WebAssembly build of the application in {} (set SDOE_WEB_APP)", app.display());
    }
    // Files of the application build take precedence over web/static.
    let mut sources: Vec<(String, Vec<u8>)> = Vec::new();
    for p in app_files.iter().chain(files(&statics).iter()) {
        println!("cargo:rerun-if-changed={}", p.display());
        let name = p.file_name().unwrap().to_string_lossy().into_owned();
        if name == "missing.html" || sources.iter().any(|(n, _)| *n == name) {
            continue;
        }
        sources.push((name, std::fs::read(p).unwrap()));
    }
    if !built {
        sources.retain(|(n, _)| n != "index.html");
        sources.push(("index.html".into(), std::fs::read(statics.join("missing.html")).unwrap()));
    }

    let mut table = String::from("static ASSETS: &[Asset] = &[\n");
    for (i, (name, data)) in sources.iter().enumerate() {
        let gz = out.join(format!("asset{i}.gz"));
        let mut enc = GzEncoder::new(Vec::new(), Compression::best());
        enc.write_all(data).unwrap();
        std::fs::write(&gz, enc.finish().unwrap()).unwrap();
        table += &format!(
            "    Asset {{ path: {:?}, mime: {:?}, etag: \"\\\"{:016x}\\\"\", gzip: include_bytes!({:?}) }},\n",
            format!("/{name}"),
            mime(name),
            fnv(data),
            gz.display().to_string()
        );
    }
    table += "];\n";
    std::fs::write(out.join("assets.rs"), table).unwrap();
}
