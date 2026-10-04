#[cfg(feature = "qml")]
use cxx_qt_build::{CxxQtBuilder, QmlModule};

fn main() {
    #[cfg(feature = "qml")]
    CxxQtBuilder::new_qml_module(QmlModule::new("VdiscAppliance"))
        .qt_module("Qml")
        .files(["src/qobject.rs"])
        .build();
}
