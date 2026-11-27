//! QML-facing objects. The engine from the `sdoe-core` crate is stored in a
//! process-wide mutex. `Backend` contains the QML entry points for JSON
//! commands and the file system code; `SheetModel` is a Qt table model over
//! engine spreadsheets.

use crate::engine;
use core::pin::Pin;
use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::{Orientation, QByteArray, QHash, QHashPair_i32_QByteArray, QModelIndex, QString, QUrl, QVariant, QVector};
use sdoe::table::Table;
use sdoe::sheet::{self, Sheet};
use serde_json::{Value, json};

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++Qt" {
        include!(<QtCore/QAbstractTableModel>);
        #[qobject]
        type QAbstractTableModel;
    }

    unsafe extern "C++" {
        include!("cxx-qt-lib/qhash.h");
        type QHash_i32_QByteArray = cxx_qt_lib::QHash<cxx_qt_lib::QHashPair_i32_QByteArray>;
        include!("cxx-qt-lib/qvariant.h");
        type QVariant = cxx_qt_lib::QVariant;
        include!("cxx-qt-lib/qmodelindex.h");
        type QModelIndex = cxx_qt_lib::QModelIndex;
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
        include!("cxx-qt-lib/qbytearray.h");
        type QByteArray = cxx_qt_lib::QByteArray;
        include!("cxx-qt-lib/qvector.h");
        type QVector_i32 = cxx_qt_lib::QVector<i32>;
        include!("cxx-qt-lib/qt.h");
        #[namespace = "Qt"]
        type Orientation = cxx_qt_lib::Orientation;
    }

    unsafe extern "C++" {
        include!("cpp/webfile.h");
        #[cxx_name = "sdoeOpenFileContent"]
        fn open_file_content(filter: &QString, done: fn(name: &QString, content: &QByteArray));
        #[cxx_name = "sdoeSaveFileContent"]
        fn save_file_content(name: &QString, content: &QByteArray);
    }

    extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[qml_singleton]
        #[qproperty(bool, busy)]
        #[qproperty(i32, revision)]
        type Backend = super::BackendRust;

        #[qobject]
        #[base = QAbstractTableModel]
        #[qml_element]
        #[qproperty(QString, sheet)]
        #[qproperty(QString, columns)]
        type SheetModel = super::SheetModelRust;

        #[qobject]
        #[base = QAbstractTableModel]
        #[qml_element]
        #[qproperty(QString, source)]
        type ResultModel = super::ResultModelRust;
    }

    impl cxx_qt::Threading for Backend {}

    unsafe extern "RustQt" {
        /// Emitted after any command that changed the project.
        #[qsignal]
        fn changed(self: Pin<&mut Backend>);

        /// Emitted when a background command finishes.
        #[qsignal]
        #[cxx_name = "asyncDone"]
        fn async_done(self: Pin<&mut Backend>, token: i32, result: QString);

        #[qinvokable]
        fn call(self: Pin<&mut Backend>, method: &QString, params: &QString) -> QString;

        #[qinvokable]
        #[cxx_name = "callAsync"]
        fn call_async(self: Pin<&mut Backend>, method: &QString, params: &QString) -> i32;

        /// Like `callAsync`, without the busy state: for refreshes that run
        /// while the interface stays usable.
        #[qinvokable]
        #[cxx_name = "callBackground"]
        fn call_background(self: Pin<&mut Backend>, method: &QString, params: &QString) -> i32;

        #[qinvokable]
        #[cxx_name = "readFile"]
        fn read_file(self: &Backend, url: &QString) -> QString;

        #[qinvokable]
        #[cxx_name = "writeFile"]
        fn write_file(self: &Backend, url: &QString, text: &QString) -> QString;

        #[qinvokable]
        #[cxx_name = "fileName"]
        fn file_name(self: &Backend, url: &QString) -> QString;

        #[qinvokable]
        #[cxx_name = "startupFile"]
        fn startup_file(self: &Backend) -> QString;

        /// Development aid: directory for automatic page screenshots
        /// (environment variable SDOE_SCREENSHOT_DIR), empty when unset.
        #[qinvokable]
        #[cxx_name = "screenshotDir"]
        fn screenshot_dir(self: &Backend) -> QString;

        /// Whether files go through the browser (the WebAssembly build, or
        /// the environment variable SDOE_WEB_FILES as a development aid).
        #[qinvokable]
        #[cxx_name = "webFiles"]
        fn web_files(self: &Backend) -> bool;

        /// Asks for a file through the browser; `contentOpened` delivers it
        /// with the given purpose.
        #[qinvokable]
        #[cxx_name = "openContent"]
        fn open_content(self: Pin<&mut Backend>, filter: &QString, purpose: &QString);

        /// Offers text as a download.
        #[qinvokable]
        #[cxx_name = "saveContent"]
        fn save_content(self: &Backend, name: &QString, text: &QString);

        /// A file from `openContent`: its name and decoded text.
        #[qsignal]
        #[cxx_name = "contentOpened"]
        fn content_opened(self: Pin<&mut Backend>, purpose: QString, name: QString, text: QString);
    }

    unsafe extern "RustQt" {
        #[inherit]
        #[cxx_name = "beginResetModel"]
        unsafe fn begin_reset_model(self: Pin<&mut SheetModel>);
        #[inherit]
        #[cxx_name = "endResetModel"]
        unsafe fn end_reset_model(self: Pin<&mut SheetModel>);
        #[inherit]
        #[qsignal]
        #[cxx_name = "dataChanged"]
        fn data_changed(self: Pin<&mut SheetModel>, top_left: &QModelIndex, bottom_right: &QModelIndex, roles: &QVector_i32);
        #[inherit]
        fn index(self: &SheetModel, row: i32, column: i32, parent: &QModelIndex) -> QModelIndex;

        /// Reloads the sheet from the engine.
        #[qinvokable]
        fn refresh(self: Pin<&mut SheetModel>);

        #[qinvokable]
        #[cxx_name = "cellText"]
        fn cell_text(self: &SheetModel, row: i32, column: i32) -> QString;

        #[qinvokable]
        #[cxx_override]
        fn data(self: &SheetModel, index: &QModelIndex, role: i32) -> QVariant;
        #[qinvokable]
        #[cxx_override]
        #[cxx_name = "roleNames"]
        fn role_names(self: &SheetModel) -> QHash_i32_QByteArray;
        #[qinvokable]
        #[cxx_override]
        #[cxx_name = "rowCount"]
        fn row_count(self: &SheetModel, parent: &QModelIndex) -> i32;
        #[qinvokable]
        #[cxx_override]
        #[cxx_name = "columnCount"]
        fn column_count(self: &SheetModel, parent: &QModelIndex) -> i32;
        #[qinvokable]
        #[cxx_override]
        #[cxx_name = "headerData"]
        fn header_data(self: &SheetModel, section: i32, orientation: Orientation, role: i32) -> QVariant;
    }

    unsafe extern "RustQt" {
        #[inherit]
        #[cxx_name = "beginResetModel"]
        unsafe fn begin_reset_model(self: Pin<&mut ResultModel>);
        #[inherit]
        #[cxx_name = "endResetModel"]
        unsafe fn end_reset_model(self: Pin<&mut ResultModel>);

        /// Tab-separated text of the whole table including the header.
        #[qinvokable]
        fn tsv(self: &ResultModel) -> QString;
        #[qinvokable]
        fn tag(self: &ResultModel, row: i32) -> QString;
        #[qinvokable]
        #[cxx_name = "widthHint"]
        fn width_hint(self: &ResultModel, column: i32) -> i32;

        #[qinvokable]
        #[cxx_override]
        fn data(self: &ResultModel, index: &QModelIndex, role: i32) -> QVariant;
        #[qinvokable]
        #[cxx_override]
        #[cxx_name = "roleNames"]
        fn role_names(self: &ResultModel) -> QHash_i32_QByteArray;
        #[qinvokable]
        #[cxx_override]
        #[cxx_name = "rowCount"]
        fn row_count(self: &ResultModel, parent: &QModelIndex) -> i32;
        #[qinvokable]
        #[cxx_override]
        #[cxx_name = "columnCount"]
        fn column_count(self: &ResultModel, parent: &QModelIndex) -> i32;
        #[qinvokable]
        #[cxx_override]
        #[cxx_name = "headerData"]
        fn header_data(self: &ResultModel, section: i32, orientation: Orientation, role: i32) -> QVariant;
    }

    impl cxx_qt::Initialize for ResultModel {}
}

#[derive(Default)]
pub struct BackendRust {
    busy: bool,
    revision: i32,
    next_token: i32,
    /// Number of `callAsync` commands that have not finished; `busy` holds
    /// while it is above zero.
    running: i32,
}

fn changed_flag(out: &str) -> bool {
    serde_json::from_str::<Value>(out).ok().and_then(|v| v.get("changed").and_then(Value::as_bool)).unwrap_or(false)
}

/// Decodes file contents: UTF-8 (with or without BOM), otherwise Latin-1.
fn decode_text(bytes: Vec<u8>) -> String {
    match String::from_utf8(bytes) {
        Ok(s) => s.trim_start_matches('\u{feff}').to_string(),
        Err(e) => e.into_bytes().iter().map(|&b| b as char).collect(),
    }
}

/// Runs a command off the GUI thread. The WebAssembly build has no threads
/// and runs it directly; its result still arrives through the event loop.
fn run_command(job: impl FnOnce() + Send + 'static) {
    #[cfg(target_os = "emscripten")]
    job();
    #[cfg(not(target_os = "emscripten"))]
    std::thread::spawn(job);
}

/// The purpose of the pending `openContent` request and the Backend that
/// receives the file.
static PENDING_OPEN: std::sync::Mutex<Option<(String, cxx_qt::CxxQtThread<qobject::Backend>)>> = std::sync::Mutex::new(None);

fn content_ready(name: &QString, content: &QByteArray) {
    let Some((purpose, qt)) = PENDING_OPEN.lock().unwrap_or_else(|p| p.into_inner()).take() else { return };
    let name = name.to_string();
    let text = decode_text(content.as_slice().to_vec());
    let _ = qt.queue(move |this| {
        this.content_opened(QString::from(&purpose), QString::from(&name), QString::from(&text));
    });
}

fn local_path(url: &QString) -> String {
    let s = url.to_string();
    if s.starts_with("file:") {
        QUrl::from(url).to_local_file_or_default().to_string()
    } else {
        s
    }
}

impl qobject::Backend {
    fn call(mut self: Pin<&mut Self>, method: &QString, params: &QString) -> QString {
        let out = engine().call(&method.to_string(), &params.to_string());
        if changed_flag(&out) {
            let r = self.revision + 1;
            self.as_mut().set_revision(r);
            self.as_mut().changed();
        }
        QString::from(&out)
    }

    fn call_async(mut self: Pin<&mut Self>, method: &QString, params: &QString) -> i32 {
        let running = self.running + 1;
        self.as_mut().rust_mut().running = running;
        self.as_mut().set_busy(true);
        self.start_command(method, params, true)
    }

    fn call_background(self: Pin<&mut Self>, method: &QString, params: &QString) -> i32 {
        self.start_command(method, params, false)
    }

    /// Runs a command off the GUI thread and delivers the reply through
    /// `asyncDone`; `counted` commands hold the busy state until they finish.
    fn start_command(mut self: Pin<&mut Self>, method: &QString, params: &QString, counted: bool) -> i32 {
        let token = self.next_token + 1;
        self.as_mut().rust_mut().next_token = token;
        let qt = self.qt_thread();
        let (m, p) = (method.to_string(), params.to_string());
        run_command(move || {
            let out = engine().call(&m, &p);
            let changed = changed_flag(&out);
            let _ = qt.queue(move |mut this| {
                if changed {
                    let r = this.revision + 1;
                    this.as_mut().set_revision(r);
                    this.as_mut().changed();
                }
                if counted {
                    let running = (this.running - 1).max(0);
                    this.as_mut().rust_mut().running = running;
                    this.as_mut().set_busy(running > 0);
                }
                this.as_mut().async_done(token, QString::from(&out));
            });
        });
        token
    }

    fn read_file(&self, url: &QString) -> QString {
        let path = local_path(url);
        let out = match std::fs::read(&path) {
            Ok(bytes) => json!({ "ok": true, "text": decode_text(bytes), "path": path }),
            Err(e) => json!({ "ok": false, "error": format!("{path}: {e}") }),
        };
        QString::from(&out.to_string())
    }

    fn write_file(&self, url: &QString, text: &QString) -> QString {
        let path = local_path(url);
        let tmp = format!("{path}.tmp-sdoe");
        let res = std::fs::write(&tmp, text.to_string()).and_then(|_| std::fs::rename(&tmp, &path));
        match res {
            Ok(()) => QString::default(),
            Err(e) => {
                let _ = std::fs::remove_file(&tmp);
                QString::from(&format!("{path}: {e}"))
            }
        }
    }

    fn startup_file(&self) -> QString {
        match std::env::var("SDOE_OPENED_FILE") {
            Ok(p) => QUrl::from_local_file(&QString::from(&p)).to_qstring(),
            Err(_) => QString::default(),
        }
    }

    fn screenshot_dir(&self) -> QString {
        QString::from(&std::env::var("SDOE_SCREENSHOT_DIR").unwrap_or_default())
    }

    fn web_files(&self) -> bool {
        cfg!(target_os = "emscripten") || std::env::var_os("SDOE_WEB_FILES").is_some()
    }

    fn open_content(self: Pin<&mut Self>, filter: &QString, purpose: &QString) {
        *PENDING_OPEN.lock().unwrap_or_else(|p| p.into_inner()) = Some((purpose.to_string(), self.qt_thread()));
        qobject::open_file_content(filter, content_ready);
    }

    fn save_content(&self, name: &QString, text: &QString) {
        qobject::save_file_content(name, &QByteArray::from(text.to_string().as_str()));
    }

    fn file_name(&self, url: &QString) -> QString {
        let path = local_path(url);
        let name = std::path::Path::new(&path).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        QString::from(&name)
    }
}

#[derive(Default)]
pub struct SheetModelRust {
    sheet: QString,
    columns: QString,
    data: Sheet,
}

const ROLE_DISPLAY: i32 = 0;
const ROLE_STATE: i32 = 257;
const ROLE_EDITABLE: i32 = 258;

impl qobject::SheetModel {
    fn refresh(mut self: Pin<&mut Self>) {
        let name = self.sheet.to_string();
        let new = {
            let eng = engine();
            sheet::get(&eng.project, &name).unwrap_or_default()
        };
        let same_shape = new.rows.len() == self.data.rows.len()
            && new.columns.len() == self.data.columns.len()
            && new.columns.iter().zip(&self.data.columns).all(|(a, b)| a.title == b.title && a.editor == b.editor && a.choices == b.choices);
        let cols = serde_json::to_string(&new.columns).unwrap_or_default();
        if same_shape {
            self.as_mut().rust_mut().data = new;
            let rows = self.data.rows.len() as i32;
            let ncols = self.data.columns.len() as i32;
            if rows > 0 && ncols > 0 {
                let tl = self.index(0, 0, &QModelIndex::default());
                let br = self.index(rows - 1, ncols - 1, &QModelIndex::default());
                self.as_mut().data_changed(&tl, &br, &QVector::<i32>::default());
            }
        } else {
            unsafe {
                self.as_mut().begin_reset_model();
            }
            self.as_mut().rust_mut().data = new;
            unsafe {
                self.as_mut().end_reset_model();
            }
        }
        if self.columns.to_string() != cols {
            self.as_mut().set_columns(QString::from(&cols));
        }
    }

    fn cell_text(&self, row: i32, column: i32) -> QString {
        self.data
            .rows
            .get(row as usize)
            .and_then(|r| r.get(column as usize))
            .map(|c| QString::from(&c.text))
            .unwrap_or_default()
    }

    fn data(&self, index: &QModelIndex, role: i32) -> QVariant {
        let Some(c) = self.data.rows.get(index.row() as usize).and_then(|r| r.get(index.column() as usize)) else {
            return QVariant::default();
        };
        match role {
            ROLE_DISPLAY => QVariant::from(&QString::from(&c.text)),
            ROLE_STATE => QVariant::from(&QString::from(c.state)),
            ROLE_EDITABLE => QVariant::from(&c.editable),
            _ => QVariant::default(),
        }
    }

    fn role_names(&self) -> QHash<QHashPair_i32_QByteArray> {
        let mut h = QHash::<QHashPair_i32_QByteArray>::default();
        h.insert(ROLE_DISPLAY, QByteArray::from("display"));
        h.insert(ROLE_STATE, QByteArray::from("cellState"));
        h.insert(ROLE_EDITABLE, QByteArray::from("editable"));
        h
    }

    fn row_count(&self, _p: &QModelIndex) -> i32 {
        self.data.rows.len() as i32
    }

    fn column_count(&self, _p: &QModelIndex) -> i32 {
        self.data.columns.len() as i32
    }

    fn header_data(&self, section: i32, orientation: Orientation, role: i32) -> QVariant {
        if role != ROLE_DISPLAY {
            return QVariant::default();
        }
        if orientation == Orientation::Horizontal {
            return self.data.columns.get(section as usize).map(|c| QVariant::from(&QString::from(&c.title))).unwrap_or_default();
        }
        let last = self.data.rows.len() as i32 - 1;
        let label = if self.data.placeholder && section == last { "+".to_string() } else { (section + 1).to_string() };
        QVariant::from(&QString::from(&label))
    }
}

#[derive(Default)]
pub struct ResultModelRust {
    source: QString,
    table: Table,
}

const ROLE_EMPHASIS: i32 = 259;
const ROLE_NUMERIC: i32 = 260;

impl cxx_qt::Initialize for qobject::ResultModel {
    fn initialize(self: Pin<&mut Self>) {
        self.on_source_changed(|mut this| {
            let t: Table = serde_json::from_str(&this.source.to_string()).unwrap_or_default();
            unsafe {
                this.as_mut().begin_reset_model();
            }
            this.as_mut().rust_mut().table = t;
            unsafe {
                this.as_mut().end_reset_model();
            }
        })
        .release();
    }
}

impl qobject::ResultModel {
    fn tsv(&self) -> QString {
        let mut s = self.table.columns.join("\t");
        s.push('\n');
        for r in &self.table.rows {
            s.push_str(&r.join("\t"));
            s.push('\n');
        }
        QString::from(&s)
    }

    fn tag(&self, row: i32) -> QString {
        self.table.tags.get(row as usize).map(|t| QString::from(t)).unwrap_or_default()
    }

    /// Character count of the widest cell in a column (header included).
    fn width_hint(&self, column: i32) -> i32 {
        let c = column as usize;
        let head = self.table.columns.get(c).map(|s| s.chars().count()).unwrap_or(0);
        self.table.rows.iter().filter_map(|r| r.get(c)).map(|s| s.chars().count()).fold(head, usize::max) as i32
    }

    fn data(&self, index: &QModelIndex, role: i32) -> QVariant {
        let (r, c) = (index.row() as usize, index.column() as usize);
        match role {
            ROLE_DISPLAY => self.table.rows.get(r).and_then(|x| x.get(c)).map(|s| QVariant::from(&QString::from(s))).unwrap_or_default(),
            ROLE_EMPHASIS => QVariant::from(&QString::from(self.table.emphasis.get(r).map(String::as_str).unwrap_or(""))),
            ROLE_NUMERIC => QVariant::from(&self.table.numeric.get(c).copied().unwrap_or(false)),
            _ => QVariant::default(),
        }
    }

    fn role_names(&self) -> QHash<QHashPair_i32_QByteArray> {
        let mut h = QHash::<QHashPair_i32_QByteArray>::default();
        h.insert(ROLE_DISPLAY, QByteArray::from("display"));
        h.insert(ROLE_EMPHASIS, QByteArray::from("emphasis"));
        h.insert(ROLE_NUMERIC, QByteArray::from("numeric"));
        h
    }

    fn row_count(&self, _p: &QModelIndex) -> i32 {
        self.table.rows.len() as i32
    }

    fn column_count(&self, _p: &QModelIndex) -> i32 {
        self.table.columns.len() as i32
    }

    fn header_data(&self, section: i32, orientation: Orientation, role: i32) -> QVariant {
        if role != ROLE_DISPLAY || orientation != Orientation::Horizontal {
            return QVariant::default();
        }
        self.table.columns.get(section as usize).map(|c| QVariant::from(&QString::from(c))).unwrap_or_default()
    }
}
