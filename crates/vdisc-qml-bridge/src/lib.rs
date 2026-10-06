#[cfg(feature = "qml")]
mod audio_backend;
mod lcd_feedback;
mod projection;
#[cfg(feature = "qml")]
mod qobject;
mod runtime;
mod scan_target;
#[cfg(feature = "qml-test-support")]
mod test_audio;

pub use projection::ApplianceSnapshot;
pub use runtime::{ApplianceRuntime, Command};
