//! Shared test helpers (test builds only): a file logger under `target/`.
use crate::{LogState, Logger};
use std::fs::File;
use std::sync::Mutex;

pub(crate) fn logger(name: &str) -> Logger {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("target")
        .join(format!("test-{name}.log"));
    Logger(Mutex::new(LogState {
        directory: path.parent().map(std::path::Path::to_owned),
        file: Some(File::create(path).expect("test log")),
        lines: 0,
        background_lines: 0,
        bytes: 0,
        rotating: false,
    }))
}

#[cfg(test)]
mod tests {
    use crate::{LogState, Logger};
    use std::sync::Mutex;

    #[test]
    fn unavailable_file_diagnostics_do_not_abort_control_callers() {
        let logger = Logger(Mutex::new(LogState {
            file: None,
            directory: None,
            lines: 0,
            background_lines: 0,
            bytes: 0,
            rotating: false,
        }));
        logger.write("control enabled without a file logger");
        crate::native_adapter::capture_trace("diagnostics unavailable", &logger);
        logger.next_session();
        assert!(logger.directory().is_none());
        assert_eq!(logger.0.lock().unwrap().lines, 0);
    }
}
