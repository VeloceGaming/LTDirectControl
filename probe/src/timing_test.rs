//! Shared test logger. Previous SDK-callback pacing has been removed.
pub(crate) mod tests {
    use crate::{LogState, Logger};
    use std::fs::File;
    use std::sync::Mutex;
    pub(crate) fn logger(name: &str) -> Logger {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join(format!("timing-{name}.log"));
        Logger(Mutex::new(LogState {
            file: File::create(path).expect("test log"),
            lines: 0,
            background_lines: 0,
        }))
    }
}
