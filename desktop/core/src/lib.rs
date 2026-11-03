pub mod ai;
pub mod canvas;
pub mod model;
pub mod store;
pub mod workload;

pub use ai::{Config, Router};
pub use model::*;
pub use store::Store;

pub mod organization;

/// The system curl, spawned without flashing a console window on Windows.
pub(crate) fn curl_command() -> std::process::Command {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        let mut command = std::process::Command::new("curl.exe");
        command.creation_flags(CREATE_NO_WINDOW);
        command
    }
    #[cfg(not(windows))]
    {
        std::process::Command::new("curl")
    }
}

#[cfg(test)]
mod tests {
    #[test]
    #[cfg(not(windows))]
    fn curl_command_uses_path_on_unix() {
        assert_eq!(super::curl_command().get_program(), "curl");
    }
}
