//! Launch background tools without allocating a Windows console.
use std::{ffi::OsStr, process::Command};

pub fn command(executable: impl AsRef<OsStr>) -> Command {
    #[allow(unused_mut)]
    let mut command = Command::new(executable);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // Keep stdout/stderr available for the helper protocol and progress logs.
        command.creation_flags(0x08000000); // CREATE_NO_WINDOW
    }
    command
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    #[test]
    fn console_probe() {
        if std::env::var_os("TANGFPGA_CONSOLE_PROBE").is_none() {
            return;
        }
        #[link(name = "kernel32")]
        extern "system" {
            fn GetConsoleWindow() -> *mut std::ffi::c_void;
        }
        assert!(unsafe { GetConsoleWindow() }.is_null(), "Helper allocated a console");
        println!("helper-output");
        eprintln!("helper-error");
        std::process::exit(7);
    }

    #[test]
    fn background_tool_preserves_output_and_failure_status() {
        let output = command(std::env::current_exe().unwrap())
            .args(["--exact", "utils::process::tests::console_probe", "--nocapture"])
            .env("TANGFPGA_CONSOLE_PROBE", "1")
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(7));
        assert!(String::from_utf8_lossy(&output.stdout).contains("helper-output"));
        assert!(String::from_utf8_lossy(&output.stderr).contains("helper-error"));
    }
}
