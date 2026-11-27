//! sdoe-web: serves the WebAssembly build of the sdoe application. All
//! files are part of the executable; the application runs in the browser.

use flate2::read::GzDecoder;
use std::io::Read;
use std::net::SocketAddr;
use std::sync::Arc;
use tiny_http::{Header, Method, Request, Response, Server, StatusCode};

struct Asset {
    path: &'static str,
    mime: &'static str,
    etag: &'static str,
    gzip: &'static [u8],
}

include!(concat!(env!("OUT_DIR"), "/assets.rs"));

const USAGE: &str = "\
Usage: sdoe-web [--listen ADDRESS:PORT] [--open]

Serves the sdoe application at http://ADDRESS:PORT/ (default
127.0.0.1:8080). The application runs in the browser; projects stay on the
computer of the browser. Use --listen 0.0.0.0:8080 to serve other computers
too. --open opens the address in the default browser.";

fn header(name: &str, value: &str) -> Header {
    Header::from_bytes(name.as_bytes(), value.as_bytes()).expect("valid header")
}

fn respond(req: Request) {
    let method = req.method().clone();
    if method != Method::Get && method != Method::Head {
        let _ = req.respond(Response::empty(StatusCode(405)).with_header(header("Allow", "GET, HEAD")));
        return;
    }
    let path = req.url().split(['?', '#']).next().unwrap_or("/").to_owned();
    let path = if path == "/" { "/index.html" } else { path.as_str() };
    let Some(asset) = ASSETS.iter().find(|a| a.path == path) else {
        let _ = req.respond(Response::from_string("not found").with_status_code(404));
        return;
    };
    let value = |name: &str| {
        req.headers().iter().find(|h| h.field.as_str().as_str().eq_ignore_ascii_case(name)).map(|h| h.value.as_str().to_owned()).unwrap_or_default()
    };
    let common = [
        header("ETag", asset.etag),
        header("Cache-Control", "no-cache"),
        header("Vary", "Accept-Encoding"),
        header("X-Content-Type-Options", "nosniff"),
    ];
    if value("If-None-Match").split(',').any(|t| t.trim() == asset.etag) {
        let mut r = Response::empty(StatusCode(304));
        for h in common {
            r.add_header(h);
        }
        let _ = req.respond(r);
        return;
    }
    let gzip = value("Accept-Encoding").split(',').any(|e| e.split(';').next().unwrap_or("").trim() == "gzip");
    let body = if gzip {
        asset.gzip.to_vec()
    } else {
        let mut v = Vec::new();
        let _ = GzDecoder::new(asset.gzip).read_to_end(&mut v);
        v
    };
    let len = body.len();
    let mut r = Response::from_data(if method == Method::Head { Vec::new() } else { body }).with_header(header("Content-Type", asset.mime));
    if method == Method::Head {
        r.add_header(header("Content-Length", &len.to_string()));
    }
    if gzip {
        r.add_header(header("Content-Encoding", "gzip"));
    }
    for h in common {
        r.add_header(h);
    }
    let _ = req.respond(r);
}

fn open_browser(url: &str) {
    let result = if cfg!(target_os = "windows") {
        std::process::Command::new("cmd").args(["/C", "start", "", url]).spawn()
    } else if cfg!(target_os = "macos") {
        std::process::Command::new("open").arg(url).spawn()
    } else {
        std::process::Command::new("xdg-open").arg(url).spawn()
    };
    if let Err(e) = result {
        eprintln!("cannot open a browser: {e}");
    }
}

fn main() {
    let mut listen = String::from("127.0.0.1:8080");
    let mut open = false;
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--listen" | "-l" => match args.next() {
                Some(v) => listen = v,
                None => {
                    eprintln!("--listen needs ADDRESS:PORT\n\n{USAGE}");
                    std::process::exit(2);
                }
            },
            "--open" => open = true,
            "--version" | "-V" => {
                println!("sdoe-web {}", env!("CARGO_PKG_VERSION"));
                return;
            }
            "--help" | "-h" => {
                println!("{USAGE}");
                return;
            }
            _ => {
                eprintln!("unknown argument: {a}\n\n{USAGE}");
                std::process::exit(2);
            }
        }
    }
    let addr: SocketAddr = match listen.parse() {
        Ok(a) => a,
        Err(_) => {
            eprintln!("not an ADDRESS:PORT: {listen}");
            std::process::exit(2);
        }
    };
    let server = match Server::http(addr) {
        Ok(s) => Arc::new(s),
        Err(e) => {
            eprintln!("cannot listen on {addr}: {e}");
            std::process::exit(1);
        }
    };
    let bound = server.server_addr().to_ip().unwrap_or(addr);
    let host = if bound.ip().is_unspecified() { "localhost".to_string() } else { bound.ip().to_string() };
    let host = if host.contains(':') { format!("[{host}]") } else { host };
    let url = format!("http://{host}:{}/", bound.port());
    println!("sdoe {} is running at {url} (Ctrl+C stops it)", env!("CARGO_PKG_VERSION"));
    if open {
        open_browser(&url);
    }
    let workers: Vec<_> = (0..8)
        .map(|_| {
            let server = Arc::clone(&server);
            std::thread::spawn(move || {
                while let Ok(req) = server.recv() {
                    respond(req);
                }
            })
        })
        .collect();
    for w in workers {
        let _ = w.join();
    }
}
