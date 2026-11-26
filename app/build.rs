use cxx_qt_build::{CxxQtBuilder, QmlFile, QmlModule};

fn main() {
    let qml = [
        "qml/Main.qml",
        "qml/Theme.qml",
        "qml/Api.qml",
        "qml/SheetView.qml",
        "qml/ResultTable.qml",
        "qml/PlotView.qml",
        "qml/OptionsForm.qml",
        "qml/SetupPage.qml",
        "qml/DesignPage.qml",
        "qml/RunsPage.qml",
        "qml/AnalysisPage.qml",
        "qml/OptimizePage.qml",
        "qml/EvaluatePage.qml",
        "qml/ImportDialog.qml",
        "qml/NextRunDialog.qml",
        "qml/FactorValueEditor.qml",
        "qml/Section.qml",
    ];
    for f in qml {
        println!("cargo:rerun-if-changed={f}");
    }
    let files: Vec<QmlFile> = qml
        .iter()
        .map(|f| QmlFile::from(f).singleton(f.ends_with("Theme.qml") || f.ends_with("Api.qml")))
        .collect();
    CxxQtBuilder::new_qml_module(QmlModule::new("Sdoe").qml_files(files))
        .qt_module("Quick")
        .qt_module("QuickControls2")
        .qt_module("Widgets")
        .files(["src/bridge.rs"])
        .build();
}
