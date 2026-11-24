use super::detector::{self, ProgrammerDevice};
use crate::{
    firmware::{manifest::FirmwareManifest, validator},
    utils::errors::{invalid, Error, Result},
};
use std::{
    io::Read,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{Arc, Mutex},
    thread,
};
pub type Reporter = Arc<dyn Fn(String) + Send + Sync>;
#[derive(Clone, serde::Serialize)]
pub struct Loader {
    pub path: PathBuf,
    pub version: String,
    pub bundled: bool,
}
fn loader_command(path: &Path) -> Command {
    let mut command = crate::utils::process::command(path);
    if cfg!(target_os = "linux")
        && path
            .parent()
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
            == Some("programmer")
    {
        command.env("LD_LIBRARY_PATH", path.parent().unwrap());
    }
    command
}
pub fn locate(resources: &Path) -> Result<Loader> {
    let name = if cfg!(windows) {
        "openFPGALoader.exe"
    } else {
        "openFPGALoader"
    };
    let bundled = resources.join("programmer").join(name);
    let (path, is_bundled) = if bundled.is_file() {
        let digest = std::fs::read_to_string(resources.join("programmer/sha256.txt"))?;
        validator::verify(&bundled, digest.trim())?;
        (bundled, true)
    } else {
        let path=std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default()).filter(|p|p.is_absolute()).map(|p|p.join(name)).find(|p|p.is_file()).ok_or_else(||Error::Hardware("openFPGALoader is missing. Install it or use a release with a bundled programmer.".into()))?;
        (path, false)
    };
    let output = loader_command(&path).arg("-V").output()?;
    let version = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    if !output.status.success() || !version.to_lowercase().contains("openfpgaloader") {
        return Err(invalid("Executable did not identify as openFPGALoader"));
    }
    let help = loader_command(&path).arg("--help").output()?;
    let help = format!(
        "{}{}",
        String::from_utf8_lossy(&help.stdout),
        String::from_utf8_lossy(&help.stderr)
    );
    for required in [
        "--busdev-num",
        "--detect",
        "--write-flash",
        "--write-sram",
        "--verify",
    ] {
        if !help.contains(required) {
            return Err(invalid(format!(
                "Programmer lacks required option {required}; use a compatible release"
            )));
        }
    }
    let boards = loader_command(&path).arg("--list-boards").output()?;
    if !boards.status.success() || !String::from_utf8_lossy(&boards.stdout).contains("tangnano20k")
    {
        return Err(invalid("Programmer does not support tangnano20k"));
    }
    Ok(Loader {
        path: path.canonicalize()?,
        version: version.trim().into(),
        bundled: is_bundled,
    })
}
pub fn args(device: &ProgrammerDevice, persistent: bool, bitstream: &Path) -> Vec<String> {
    let mut args = vec![
        "-b".into(),
        "tangnano20k".into(),
        "--busdev-num".into(),
        device.id.clone(),
        if persistent { "-f".into() } else { "-m".into() },
    ];
    if persistent {
        args.push("--verify".into());
    }
    args.push(bitstream.to_string_lossy().into_owned());
    args
}
pub trait ProcessRunner {
    fn run(&self, exe: &Path, args: &[String], report: Reporter) -> Result<String>;
}
pub struct NativeRunner;
impl ProcessRunner for NativeRunner {
    fn run(&self, exe: &Path, args: &[String], report: Reporter) -> Result<String> {
        let mut child = loader_command(exe)
            .args(args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .stdin(Stdio::null())
            .spawn()?;
        let combined = Arc::new(Mutex::new(String::new()));
        let readers: Vec<Box<dyn Read + Send>> = vec![
            Box::new(child.stdout.take().unwrap()),
            Box::new(child.stderr.take().unwrap()),
        ];
        let threads: Vec<_> = readers
            .into_iter()
            .map(|mut reader| {
                let combined = combined.clone();
                let report = report.clone();
                thread::spawn(move || {
                    let mut buffer = [0; 2048];
                    loop {
                        let n = reader.read(&mut buffer)?;
                        if n == 0 {
                            break;
                        }
                        let text = String::from_utf8_lossy(&buffer[..n]).to_string();
                        report(text.clone());
                        let mut log = combined.lock().unwrap();
                        log.push_str(&text);
                        if log.len() > 256 * 1024 {
                            let mut cut = log.len() - 256 * 1024;
                            while !log.is_char_boundary(cut) {
                                cut += 1;
                            }
                            log.drain(..cut);
                        }
                    }
                    Ok::<_, std::io::Error>(())
                })
            })
            .collect();
        let status = child.wait()?;
        for thread in threads {
            thread
                .join()
                .map_err(|_| Error::Hardware("Programmer output reader failed".into()))??;
        }
        let text = combined.lock().unwrap().clone();
        if !status.success() {
            return Err(Error::Hardware(format!("Programming process failed ({status}). Check the activity log and retain any backups before retrying.\n{text}")));
        }
        Ok(text)
    }
}
pub fn program(
    runner: &dyn ProcessRunner,
    loader: &Loader,
    device: &ProgrammerDevice,
    manifest: &FirmwareManifest,
    bitstream: &Path,
    persistent: bool,
    report: Reporter,
) -> Result<()> {
    manifest.validate()?;
    validator::verify(bitstream, &manifest.sha256)?;
    let fresh = detector::scan()?
        .into_iter()
        .find(|d| d.id == device.id && d.serial == device.serial)
        .ok_or_else(|| Error::Hardware("Selected programmer disconnected or changed".into()))?;
    if !fresh.accessible {
        return Err(Error::Hardware(
            "USB programmer cannot be opened. Check driver/udev permissions.".into(),
        ));
    }
    let detect = runner.run(
        &loader.path,
        &[
            "-b".into(),
            "tangnano20k".into(),
            "--busdev-num".into(),
            device.id.clone(),
            "--detect".into(),
        ],
        report.clone(),
    )?;
    if !["GW2A-18", "GW2AR-18", "GW2A(R)-18"]
        .iter()
        .any(|s| detect.contains(s))
    {
        return Err(Error::Hardware(
            "JTAG did not report a supported GW2A(R)-18 FPGA. Programming refused.".into(),
        ));
    }
    report("Compatible FPGA detected. Starting programming; keep USB connected.".into());
    runner.run(&loader.path, &args(device, persistent, bitstream), report)?;
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    fn dev() -> ProgrammerDevice {
        ProgrammerDevice {
            id: "1:2".into(),
            serial: None,
            description: "test".into(),
            accessible: true,
            identification: "test".into(),
        }
    }
    #[test]
    fn fixed_programming_commands() {
        let a = args(&dev(), true, Path::new("/tmp/test.fs"));
        assert!(a.contains(&"-f".into()));
        assert!(a.contains(&"--verify".into()));
        assert!(!a.contains(&"--dfu".into()));
        let a = args(&dev(), false, Path::new("/tmp/test.fs"));
        assert!(a.contains(&"-m".into()));
        assert!(!a.contains(&"-f".into()));
    }
    #[cfg(unix)]
    #[test]
    fn failed_process_is_not_success() {
        assert!(NativeRunner
            .run(Path::new("/usr/bin/false"), &[], Arc::new(|_| {}))
            .is_err());
    }
}
