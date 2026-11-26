//! sdoe desktop application (Qt Quick). The design-of-experiments engine is
//! the `sdoe-core` crate; this crate provides the user interface and file
//! access.

mod bridge;

use cxx_qt_lib::{QQmlApplicationEngine, QString, QUrl};
use cxx_qt_lib_extras::QApplication;
use sdoe::Engine;
use std::sync::{LazyLock, Mutex, MutexGuard};

static ENGINE: LazyLock<Mutex<Engine>> = LazyLock::new(|| Mutex::new(Engine::new()));

/// Locks the process-wide engine.
pub fn engine() -> MutexGuard<'static, Engine> {
    ENGINE.lock().unwrap_or_else(|p| p.into_inner())
}

fn main() {
    // QApplication (rather than QGuiApplication) lets desktop Qt Quick styles
    // that draw with QStyle, such as KDE's org.kde.desktop, load.
    let mut app = QApplication::new();
    if let Some(mut a) = app.as_mut() {
        a.as_mut().set_application_name(&QString::from("sdoe"));
        a.as_mut().set_organization_name(&QString::from("sdoe"));
        a.as_mut().set_application_display_name(&QString::from("sdoe"));
        a.as_mut().set_application_version(&QString::from(env!("CARGO_PKG_VERSION")));
    }
    // Opens a project file given on the command line.
    if let Some(path) = std::env::args().nth(1) {
        match std::fs::read_to_string(&path) {
            Ok(text) => {
                let params = serde_json::json!({ "text": text }).to_string();
                let out = engine().call("project.open", &params);
                if !out.contains("\"ok\":true") {
                    eprintln!("{path}: {out}");
                }
                unsafe { std::env::set_var("SDOE_OPENED_FILE", &path) };
            }
            Err(e) => eprintln!("{path}: {e}"),
        }
    }
    let mut qml = QQmlApplicationEngine::new();
    if let Some(qml) = qml.as_mut() {
        qml.load(&QUrl::from("qrc:/qt/qml/Sdoe/qml/Main.qml"));
    }
    if let Some(app) = app.as_mut() {
        app.exec();
    }
}
