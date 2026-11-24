use crate::{
    firmware::{
        downloader,
        manifest::{FirmwareManifest, BOARD},
        nanoblast::{NanoRelease, REPOSITORY},
        validator,
    },
    hardware::{
        detector::ProgrammerDevice,
        programmer::{self, Reporter},
    },
    storage::{
        detector::{revalidate, NativeStorage, StorageAdapter, StorageDevice},
        formatter, installer,
        installer::SdManifest,
        roms,
    },
    utils::{
        errors::{invalid, Error, Result},
        paths::confined,
    },
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
};
use tauri::{Emitter, Manager};
#[derive(Clone, Serialize, Deserialize, Default)]
struct Saved {
    repository: String,
    history: Vec<History>,
}
#[derive(Clone, Serialize, Deserialize)]
struct History {
    version: String,
    device: String,
    mode: String,
    timestamp: u64,
}
#[derive(Clone)]
struct SelectedFirmware {
    manifest: FirmwareManifest,
    path: PathBuf,
}
#[derive(Clone, Serialize)]
struct FirmwareView {
    manifest: FirmwareManifest,
    path: String,
}
#[derive(Default)]
struct Session {
    firmware: Option<SelectedFirmware>,
    nano: Option<(NanoRelease, PathBuf)>,
    selected_setup: Option<Value>,
    sd: Option<(SdManifest, PathBuf)>,
    devices: Vec<ProgrammerDevice>,
    storage: Vec<StorageDevice>,
    saved: Saved,
    mock_disconnected: bool,
    mock_fail: bool,
    mock_no_card: bool,
}
struct State {
    session: Mutex<Session>,
    busy: AtomicBool,
    cancel: AtomicBool,
    simulation: bool,
    data: PathBuf,
    resources: PathBuf,
}
#[derive(Clone, Serialize)]
struct Snapshot {
    pending_install: Option<Value>,
    simulation: bool,
    devices: Vec<ProgrammerDevice>,
    storage: Vec<StorageDevice>,
    loader: Option<programmer::Loader>,
    warnings: Vec<String>,
    firmware: Option<FirmwareView>,
    nanoblast: Option<NanoRelease>,
    setup_firmware: Option<Value>,
    sd: Option<SdManifest>,
    repository: String,
    history: Vec<History>,
    busy: bool,
}
#[derive(Clone, Serialize)]
struct Event {
    message: String,
    kind: String,
}
// The desktop IPC boundary accepts firmware-update actions only.
#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
enum FlasherAction {
    PrepareSystem { port: String, identity: String, confirmation: String },
    ListInstallProgrammers,
    ProgramSystemFpga { device_id: String, serial: Option<String>, confirmation: String },
    FinishSystem { port: String, identity: String, confirmation: String },
    SelectSetupRelease { path: String },
    InstallSetup { port: String, identity: String, confirmation: String },
    ListMenuPorts,
    Simulate { scenario: String },
}
impl From<FlasherAction> for Action {
    fn from(value: FlasherAction) -> Self {
        match value {
            FlasherAction::PrepareSystem { port, identity, confirmation } => Self::PrepareSystem { port, identity, confirmation },
            FlasherAction::ListInstallProgrammers => Self::ListInstallProgrammers,
            FlasherAction::ProgramSystemFpga { device_id, serial, confirmation } => Self::ProgramSystemFpga { device_id, serial, confirmation },
            FlasherAction::FinishSystem { port, identity, confirmation } => Self::FinishSystem { port, identity, confirmation },
            FlasherAction::SelectSetupRelease { path } => Self::SelectSetupRelease { path },
            FlasherAction::InstallSetup { port, identity, confirmation } => Self::InstallSetup { port, identity, confirmation },
            FlasherAction::ListMenuPorts => Self::ListMenuPorts,
            FlasherAction::Simulate { scenario } => Self::Simulate { scenario },
        }
    }
}
#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
enum Action {
    PrepareSystem { port: String, identity: String, confirmation: String },
    ListInstallProgrammers,
    ProgramSystemFpga { device_id: String, serial: Option<String>, confirmation: String },
    FinishSystem { port: String, identity: String, confirmation: String },

    SelectSetupRelease {
        path: String,
    },
    InstallSetup {
        port: String,
        identity: String,
        confirmation: String,
    },
    InstallDevelopmentMenu {
        port: String,
        identity: String,
        confirmation: String,
        manifest_path: String,
        restore: bool,
    },
    UsbRomPorts,
    UsbRomList {
        port: String,
        identity: String,
        path: String,
    },
    UsbRomUpload {
        port: String,
        identity: String,
        directory: String,
        files: Vec<String>,
    },
    ListMenuPorts,
    InstallMenu {
        port: String,
        identity: String,
        confirmation: String,
    },
    SelectNanoRelease {
        path: String,
    },
    DownloadNanoRelease {
        url: String,
    },
    SelectFirmware {
        path: String,
    },
    SelectSdManifest {
        path: String,
    },
    CheckReleases {
        repository: String,
    },
    DownloadFirmware {
        manifest_url: String,
    },
    Program {
        device_id: String,
        persistent: bool,
        board_confirmed: bool,
    },
    Prepare {
        device_id: String,
    },
    Format {
        device_id: String,
        confirmation: String,
        expected_identity: String,
        expected_capacity: u64,
    },
    Eject {
        device_id: String,
    },
    ListRoms {
        device_id: String,
    },
    ImportRoms {
        device_id: String,
        files: Vec<String>,
    },
    RemoveRom {
        device_id: String,
        path: String,
        confirmed: bool,
    },
    Simulate {
        scenario: String,
    },
}
fn event(app: &tauri::AppHandle, message: impl Into<String>, kind: &str) {
    let e = Event {
        message: message.into(),
        kind: kind.into(),
    };
    let _ = app.emit("operation", &e);
    if let Ok(path) = app.path().app_log_dir() {
        let _ = fs::create_dir_all(&path);
        let file = path.join("setup.log");
        if fs::metadata(&file)
            .map(|m| m.len() > 2 * 1024 * 1024)
            .unwrap_or(false)
        {
            let _ = fs::rename(&file, path.join("setup.previous.log"));
        }
        use std::io::Write;
        if let Ok(mut f) = fs::OpenOptions::new().create(true).append(true).open(file) {
            let _ = writeln!(f, "{}", serde_json::to_string(&e).unwrap_or_default());
        }
    }
}
fn saved(state: &State) -> Result<()> {
    let text = serde_json::to_vec_pretty(&state.session.lock().unwrap().saved)?;
    let temp = state.data.join("settings.tmp");
    fs::write(&temp, text)?;
    let target = state.data.join("settings.json");
    if cfg!(windows) && target.exists() {
        fs::remove_file(&target)?;
    }
    fs::rename(temp, target)?;
    Ok(())
}
fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T> {
    if fs::metadata(path)?.len() > 1024 * 1024 {
        return Err(invalid("Manifest exceeds 1 MiB"));
    }
    Ok(serde_json::from_slice(&fs::read(path)?)?)
}
fn select_firmware(state: &State, path: &Path) -> Result<Value> {
    let canonical = path.canonicalize()?;
    let (manifest, source) = if path.extension().and_then(|s| s.to_str()) == Some("fs") {
        (
            FirmwareManifest {
                version: "local-unversioned".into(),
                board: BOARD.into(),
                bitstream: canonical
                    .file_name()
                    .unwrap()
                    .to_string_lossy()
                    .into_owned(),
                download_url: None,
                sha256: validator::hash(&canonical)?,
                release_date: "unknown".into(),
                sd_asset_version: "unconfigured".into(),
            },
            canonical,
        )
    } else {
        let m: FirmwareManifest = read_json(&canonical)?;
        m.validate()?;
        let source = confined(canonical.parent().unwrap(), &m.bitstream)?;
        (m, source)
    };
    manifest.validate()?;
    if fs::metadata(&source)?.len() == 0 || fs::metadata(&source)?.len() > 64 * 1024 * 1024 {
        return Err(invalid("Invalid bitstream size"));
    }
    // Stage an immutable-to-the-UI, checksummed copy before using a user-selected file.
    let staged = state
        .data
        .join(format!("{}.fs", manifest.sha256.to_ascii_lowercase()));
    if source != staged {
        fs::copy(&source, &staged)?;
    }
    validator::verify(&staged, &manifest.sha256)?;
    let value = json!(manifest);
    state.session.lock().unwrap().firmware = Some(SelectedFirmware {
        manifest,
        path: staged,
    });
    Ok(value)
}
fn selected_device(state: &State, id: &str) -> Result<StorageDevice> {
    state
        .session
        .lock()
        .unwrap()
        .storage
        .iter()
        .find(|d| d.id == id)
        .cloned()
        .ok_or_else(|| invalid("Refresh and select an SD card first"))
}
fn assets(state: &State) -> Result<(SdManifest, PathBuf)> {
    state
        .session
        .lock()
        .unwrap()
        .sd
        .clone()
        .ok_or_else(|| invalid("Select the repository's SD asset manifest first"))
}
fn pending_request(state: &State) -> Result<PathBuf> {
    let index: Value = read_json(&state.data.join("pending-system.json"))?;
    let relative = index["request"].as_str().ok_or_else(|| invalid("Missing installation request"))?;
    if !relative.starts_with("bl616-backups/") { return Err(invalid("Invalid installation backup location")); }
    confined(&state.data, relative)
}
fn pending_view(state: &State) -> Option<Value> {
    let path = pending_request(state).ok()?;
    let mut value: Value = read_json(&path).ok()?;
    if value["phase"] == "complete" { return None; }
    value["backup"] = json!(path.parent()?.to_string_lossy());
    Some(value)
}
fn installation_step(state: &State, app: &tauri::AppHandle, mut args: Vec<String>) -> Result<Value> {
    args.extend(["--release".into(), state.resources.join("nanoblast-v0.2.0").to_string_lossy().into_owned()]);
    let handle = app.clone();
    let report: Reporter = Arc::new(move |message| event(&handle, message, "info"));
    crate::hardware::bl616::system_step(&state.resources, &args, report)
}
fn snapshot_sync(state: &State) -> Result<Snapshot> {
    // Flashing detects BL616 UPDATE ports directly; it does not inspect SD cards
    // or probe unrelated FPGA programmers.
    let warnings = Vec::new();
    let devices = Vec::new();
    let storage = Vec::new();
    let loader = None;
    let mut s = state.session.lock().unwrap();
    s.devices = devices.clone();
    s.storage = storage.clone();
    Ok(Snapshot {
        pending_install: pending_view(state),
        simulation: state.simulation,
        devices,
        storage,
        loader,
        warnings,
        firmware: s.firmware.as_ref().map(|f| FirmwareView {
            manifest: f.manifest.clone(),
            path: f.path.to_string_lossy().into_owned(),
        }),
        sd: s.sd.as_ref().map(|s| s.0.clone()),
        nanoblast: s.nano.as_ref().map(|r| r.0.clone()),
        setup_firmware: s.selected_setup.clone(),
        repository: s.saved.repository.clone(),
        history: s.saved.history.clone(),
        busy: state.busy.load(Ordering::SeqCst),
    })
}
fn execute(state: &State, action: Action, app: &tauri::AppHandle) -> Result<Value> {
    // Simulation returns before any hardware, storage, download, or file mutation code.
    if state.simulation {
        if let Action::InstallSetup { port, confirmation, .. } = &action {
            if state.session.lock().unwrap().selected_setup.is_none() {
                return Err(invalid("Choose a downloaded firmware release first"));
            }
            if confirmation != &format!("UPDATE {port}") {
                return Err(invalid("Exact UPDATE-port confirmation required"));
            }
        }

        return simulate(state, action, app);
    }
    match action {
        Action::PrepareSystem { port, identity, confirmation } => {
            if pending_view(state).is_some() { return Err(invalid("Finish the pending installation before preparing another")); }
            let selected = state.session.lock().unwrap().selected_setup.clone().ok_or_else(|| invalid("Choose a system release first"))?;
            let selected_path = selected["path"].as_str().ok_or_else(|| invalid("Selected release has no path"))?;
            let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_nanos();
            let relative = format!("bl616-backups/install-{stamp}");
            let backup = state.data.join(&relative);
            event(app, format!("Installation backups: {}", backup.display()), "info");
            let value = installation_step(state, app, vec!["--system-stage".into(),"prepare".into(),"--current-bundle".into(),selected_path.into(),"--output".into(),backup.to_string_lossy().into_owned(),"--port".into(),port,"--identity".into(),identity,"--confirmation".into(),confirmation])?;
            if value["phase"] != "primary_verified" { return Err(invalid("Primary programmer verification missing")); }
            fs::write(state.data.join("pending-system.json"), serde_json::to_vec(&json!({"request":format!("{relative}/system-install.json")}))?)?;
            Ok(value)
        }
        Action::ListInstallProgrammers => {
            let pending = pending_view(state).ok_or_else(|| invalid("No installation is waiting for a programmer"))?;
            if pending["phase"] != "primary_verified" && pending["phase"] != "fpga_pending" { return Err(invalid("Installation is not waiting for FPGA programming")); }
            Ok(json!(crate::hardware::detector::scan()?))
        }
        Action::ProgramSystemFpga { device_id, serial, confirmation } => {
            let device = crate::hardware::detector::scan()?.into_iter().find(|device| device.id == device_id && device.serial == serial)
                .ok_or_else(|| invalid("Selected programmer disconnected or changed"))?;
            if !device.accessible { return Err(invalid("Cannot open the programmer. Check the USB driver and permissions.")); }
            let request = pending_request(state)?;
            let mut args = vec!["--system-stage".into(),"fpga".into(),"--request".into(),request.to_string_lossy().into_owned(),"--busdev".into(),device_id,"--confirmation".into(),confirmation];
            if let Some(serial) = serial { args.extend(["--serial".into(),serial]); }
            let value = installation_step(state, app, args)?;
            if value["phase"] != "fpga_verified" { return Err(invalid("FPGA readback verification missing")); }
            Ok(value)
        }
        Action::FinishSystem { port, identity, confirmation } => {
            let request = pending_request(state)?;
            let value = installation_step(state, app, vec!["--system-stage".into(),"finish".into(),"--request".into(),request.to_string_lossy().into_owned(),"--port".into(),port,"--identity".into(),identity,"--confirmation".into(),confirmation,"--trusted-receipts".into(),state.data.join("verified-release-layouts.json").to_string_lossy().into_owned()])?;
            if value["phase"] != "complete" { return Err(invalid("Final controller verification missing")); }
            Ok(json!({"phase":"complete","target":value["target"],"backup":request.parent().unwrap().to_string_lossy()}))
        }
        Action::SelectSetupRelease { path } => {
            state.session.lock().unwrap().selected_setup = None;
            let stamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos();
            let source = Path::new(&path).canonicalize()?;
            let cache = state.data.join("selected-releases").join(stamp.to_string());
            let value = crate::hardware::bl616::select_release(&state.resources, &source, &cache)?;
            state.session.lock().unwrap().selected_setup = Some(value.clone());
            Ok(value)
        }
        Action::InstallSetup {
            port,
            identity,
            confirmation,
        } => {
            let selected = state
                .session
                .lock()
                .unwrap()
                .selected_setup
                .clone()
                .ok_or_else(|| invalid("Choose a downloaded firmware release first"))?;
            let selected_path = PathBuf::from(
                selected["path"]
                    .as_str()
                    .ok_or_else(|| invalid("Selected release has no directory"))?,
            );
            let stamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos();
            let backup = state
                .data
                .join("bl616-backups")
                .join(format!("setup-{stamp}"));
            event(
                app,
                format!("Full flash backup: {}", backup.display()),
                "info",
            );
            let handle = app.clone();
            let report: Reporter = Arc::new(move |s| event(&handle, s, "info"));
            crate::hardware::bl616::install_selected(
                &state.resources,
                &state.resources.join("nanoblast-v0.2.0"),
                &backup,
                &crate::hardware::bl616::Port {
                    port,
                    identity,
                    description: String::new(),
                },
                &confirmation,
                report,
                None,
                Some(&selected_path),
            )?;
            Ok(
                json!({"message": if selected["kind"] == "arcade" { "Arcade application verified. The primary programmer and FPGA recovery cache were preserved. Disconnect power, insert the prepared SD card, reconnect HDMI and power on without UPDATE." } else { "Firmware and all four cores verified. Disconnect every USB and power cable for 5 seconds, then reconnect without UPDATE and select a core." }, "backup":backup.to_string_lossy()}),
            )
        }
        Action::InstallDevelopmentMenu {
            port,
            identity,
            confirmation,
            manifest_path,
            restore,
        } => {
            let root = state.resources.join("nanoblast-v0.2.0");
            let stamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos();
            let backup = state
                .data
                .join("bl616-backups")
                .join(format!("development-{stamp}"));
            let manifest = Path::new(&manifest_path).canonicalize()?;
            let handle = app.clone();
            let report: Reporter = Arc::new(move |s| event(&handle, s, "info"));
            crate::hardware::bl616::install_selected(
                &state.resources,
                &root,
                &backup,
                &crate::hardware::bl616::Port {
                    port,
                    identity,
                    description: String::new(),
                },
                &confirmation,
                report,
                Some((&manifest, restore)),
                None,
            )?;
            Ok(
                json!({"message":format!("Selected menu verified by full flash readback. Backup: {}. Reconnect without UPDATE to boot. USB ROM transfer requires selecting that mode on the console.",backup.display())}),
            )
        }
        Action::UsbRomPorts => crate::hardware::bl616::files(
            &state.resources,
            "ports",
            "",
            "",
            "",
            None,
            Arc::new(|_| {}),
        ),
        Action::UsbRomList {
            port,
            identity,
            path,
        } => crate::hardware::bl616::files(
            &state.resources,
            "list",
            &port,
            &identity,
            &path,
            None,
            Arc::new(|_| {}),
        ),
        Action::UsbRomUpload {
            port,
            identity,
            directory,
            files,
        } => {
            if files.is_empty() || files.len() > 100 {
                return Err(invalid("Choose between 1 and 100 ROM files"));
            }
            let mut results = Vec::new();
            for file in files {
                if state.cancel.load(Ordering::Relaxed) {
                    return Err(Error::Cancelled);
                }
                let path = Path::new(&file).canonicalize()?;
                if !path.is_file() {
                    return Err(invalid("Select a ROM file"));
                }
                let name = path
                    .file_name()
                    .and_then(|s| s.to_str())
                    .ok_or_else(|| invalid("Invalid filename"))?;
                let remote = format!("{directory}/{name}");
                let handle = app.clone();
                let report: Reporter = Arc::new(move |s| event(&handle, s, "info"));
                results.push(crate::hardware::bl616::files(
                    &state.resources,
                    "upload",
                    &port,
                    &identity,
                    &remote,
                    Some(&path),
                    report,
                )?);
            }
            let paths: Vec<&str> = results.iter().filter_map(|r| r["path"].as_str()).collect();
            Ok(
                json!({"message":format!("Uploaded and verified on SD: {}. Original Mac files are unchanged.", paths.join(", ")),"files":results}),
            )
        }
        Action::ListMenuPorts => Ok(json!(crate::hardware::bl616::ports(&state.resources)?)),
        Action::InstallMenu {
            port,
            identity,
            confirmation,
        } => {
            let (release, root) = state
                .session
                .lock()
                .unwrap()
                .nano
                .clone()
                .ok_or_else(|| invalid("Select a verified NanoBlast release"))?;
            let stamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos();
            let backup = state.data.join("bl616-backups").join(stamp.to_string());
            event(
                app,
                format!(
                    "BL616 backup and verification records: {}",
                    backup.display()
                ),
                "info",
            );
            let handle = app.clone();
            let report: Reporter = Arc::new(move |s| event(&handle, s, "info"));
            crate::hardware::bl616::install(
                &state.resources,
                &root,
                &backup,
                &crate::hardware::bl616::Port {
                    port: port.clone(),
                    identity,
                    description: String::new(),
                },
                &confirmation,
                report,
            )?;
            state.session.lock().unwrap().saved.history.push(History {
                version: release.version,
                device: port,
                mode: "BL616 menu + cores (full readback verified)".into(),
                timestamp: (stamp / 1_000_000_000) as u64,
            });
            if let Err(e) = saved(state) {
                event(
                    app,
                    format!("Menu verified; history could not be saved: {e}"),
                    "warning",
                );
            }
            Ok(
                json!({"message":format!("BL616 menu and cores verified against a complete flash readback. Debugger and reserved regions preserved. Backup: {}. Disconnect USB, release UPDATE, and reconnect to boot.",backup.display())}),
            )
        }
        Action::SelectNanoRelease { path } => {
            let path = Path::new(&path).canonicalize()?;
            let (release, root) = if path.extension().and_then(|s| s.to_str()) == Some("zip") {
                crate::firmware::nanoblast::unpack(
                    &fs::read(&path)?,
                    &state.data.join("nanoblast-v0.2.0"),
                )?
            } else {
                (
                    NanoRelease::load(&path)?,
                    path.parent().unwrap().to_path_buf(),
                )
            };
            let value = json!(release);
            state.session.lock().unwrap().nano = Some((release, root));
            Ok(value)
        }
        Action::DownloadNanoRelease { url } => {
            let (release, root) =
                crate::firmware::nanoblast::download(&url, &state.data.join("nanoblast-v0.2.0"))?;
            let value = json!(release);
            state.session.lock().unwrap().nano = Some((release, root));
            Ok(value)
        }
        Action::SelectFirmware { path } => select_firmware(state, Path::new(&path)),
        Action::SelectSdManifest { path } => {
            let path = Path::new(&path).canonicalize()?;
            let m: SdManifest = read_json(&path)?;
            m.validate()?;
            state.session.lock().unwrap().sd = Some((m.clone(), path.parent().unwrap().into()));
            Ok(json!(m))
        }
        Action::CheckReleases { repository } => {
            let releases = downloader::releases(&repository)?;
            state.session.lock().unwrap().saved.repository = repository;
            saved(state)?;
            Ok(json!(releases))
        }
        Action::DownloadFirmware { manifest_url } => {
            let (m, path) =
                downloader::download_manifest(&manifest_url, &state.data.join("downloads"))?;
            let value = json!(m);
            state.session.lock().unwrap().firmware = Some(SelectedFirmware { manifest: m, path });
            Ok(value)
        }
        Action::Program {
            device_id,
            persistent,
            board_confirmed,
        } => {
            if !board_confirmed {
                return Err(invalid(
                    "Confirm the selected programmer is physically connected to your Tang Nano 20K",
                ));
            }
            let (fw, device) = {
                let s = state.session.lock().unwrap();
                (
                    s.firmware
                        .clone()
                        .ok_or_else(|| invalid("Select firmware first"))?,
                    s.devices
                        .iter()
                        .find(|d| d.id == device_id)
                        .cloned()
                        .ok_or_else(|| invalid("Select a detected programmer"))?,
                )
            };
            let loader = programmer::locate(&state.resources)?;
            let handle = app.clone();
            let report: Reporter = Arc::new(move |s| event(&handle, s, "info"));
            programmer::program(
                &programmer::NativeRunner,
                &loader,
                &device,
                &fw.manifest,
                &fw.path,
                persistent,
                report,
            )?;
            let h = History {
                version: fw.manifest.version,
                device: device.serial.unwrap_or(device.id),
                mode: if persistent {
                    "flash".into()
                } else {
                    "SRAM (temporary)".into()
                },
                timestamp: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs(),
            };
            state.session.lock().unwrap().saved.history.push(h);
            if let Err(e) = saved(state) {
                event(
                    app,
                    format!(
                        "Programming completed, but installation history could not be saved: {e}"
                    ),
                    "warning",
                );
            }
            Ok(
                json!({"message":if persistent{"Programming completed with --verify requested from openFPGALoader. Check the activity log for verification details. Running NanoBlast firmware was not independently verified."}else{"SRAM programming completed. Running firmware was not independently verified; this installation is temporary."}}),
            )
        }
        Action::Prepare { device_id } => {
            let expected = selected_device(state, &device_id)?;
            let fresh = revalidate(&NativeStorage, &expected)?;
            let root = fresh.writable()?.to_path_buf();
            let (m, source) = assets(state)?;
            {
                let session = state.session.lock().unwrap();
                if let Some((release, _)) = &session.nano {
                    m.compatible(&release.version, "nanoblast-v0.2.0-sd1")?;
                } else {
                    let fw = session
                        .firmware
                        .as_ref()
                        .ok_or_else(|| invalid("Select a release to check SD compatibility"))?;
                    m.compatible(&fw.manifest.version, &fw.manifest.sd_asset_version)?;
                }
            }
            installer::install(
                &root,
                &source,
                &m,
                fresh.available,
                &state.cancel,
                || revalidate(&NativeStorage, &expected).map(|_| ()),
                |message| event(app, message, "info"),
            )?;
            Ok(
                json!({"message":"SD layout prepared and required files verified. NanoBlast v0.2.0 embeds menu artwork and cores in BL616 flash; no system files need copying for that profile. Existing ROMs and saves preserved."}),
            )
        }
        Action::Format {
            device_id,
            confirmation,
            expected_identity,
            expected_capacity,
        } => {
            let device = selected_device(state, &device_id)?;
            if device.identity != expected_identity || device.capacity != expected_capacity {
                return Err(Error::Unsafe(
                    "The device changed while the confirmation dialog was open. Select it again."
                        .into(),
                ));
            }
            formatter::format(&NativeStorage, &device, &confirmation)?;
            Ok(
                json!({"message":"Format command completed. Refresh and reselect the volume before preparing it."}),
            )
        }
        Action::Eject { device_id } => {
            let expected = selected_device(state, &device_id)?;
            let d = revalidate(&NativeStorage, &expected)?;
            NativeStorage.eject(&d)?;
            Ok(
                json!({"message":"Unmount/eject requested. Confirm the volume is unmounted before removal."}),
            )
        }
        Action::ListRoms { device_id } => {
            let d = revalidate(&NativeStorage, &selected_device(state, &device_id)?)?;
            let (m, _) = assets(state)?;
            Ok(json!(roms::list(d.writable()?, &m)?))
        }
        Action::ImportRoms { device_id, files } => {
            let expected = selected_device(state, &device_id)?;
            let d = revalidate(&NativeStorage, &expected)?;
            let (m, _) = assets(state)?;
            Ok(json!(roms::import(
                d.writable()?,
                &m,
                &files,
                d.available,
                &state.cancel,
                || revalidate(&NativeStorage, &expected).map(|_| ())
            )?))
        }
        Action::RemoveRom {
            device_id,
            path,
            confirmed,
        } => {
            if !confirmed {
                return Err(invalid("ROM removal needs confirmation"));
            }
            let d = revalidate(&NativeStorage, &selected_device(state, &device_id)?)?;
            let (m, _) = assets(state)?;
            roms::remove(d.writable()?, &m, &path)?;
            Ok(json!({"message":"Selected ROM removed"}))
        }
        Action::Simulate { .. } => Err(invalid(
            "Simulation controls require NANOBLAST_SIMULATION=1 at launch",
        )),
    }
}
fn simulate(state: &State, action: Action, app: &tauri::AppHandle) -> Result<Value> {
    match action {
        Action::PrepareSystem { .. } | Action::ProgramSystemFpga { .. } | Action::FinishSystem { .. } | Action::ListInstallProgrammers =>
            Err(invalid("Use the isolated browser preview for first-install simulation")),

        Action::SelectSetupRelease { path } => {
            if path != "SIMULATION-RELEASE" { return Err(invalid("Preview cannot read actual release files")); }
            let release=json!({"version":"Demo release","path":"SIMULATION","source":"Demo-release.zip","bytes":667424,"sha256":"0000000000000000000000000000000000000000000000000000000000000000","cores":4,"layout":"BL616 / NBCRXZ01"});
            state.session.lock().unwrap().selected_setup=Some(release.clone());
            Ok(release)
        }

        Action::Simulate { scenario } => {
            let mut s = state.session.lock().unwrap();
            s.mock_disconnected = scenario == "disconnected";
            s.mock_fail = scenario == "failure";
            s.mock_no_card = scenario == "no_card";
            Ok(Value::Null)
        }
        Action::InstallSetup { .. }
        | Action::InstallMenu { .. }
        | Action::Program { .. }
        | Action::Prepare { .. }
        | Action::Format { .. } => {
            let (fail, disconnected, no_card) = {
                let s = state.session.lock().unwrap();
                (s.mock_fail, s.mock_disconnected, s.mock_no_card)
            };
            if disconnected || no_card {
                return Err(Error::Hardware(
                    "Simulation: required device is disconnected".into(),
                ));
            }
            for message in [
                "Simulation: validating selection",
                "Simulation: transferring data",
                "Simulation: verifying result",
            ] {
                event(app, message, "info");
                std::thread::sleep(std::time::Duration::from_millis(350));
                if state.cancel.load(Ordering::Relaxed) {
                    return Err(Error::Cancelled);
                }
            }
            if fail {
                return Err(Error::Hardware(
                    "Simulated programming failure. No real hardware was accessed.".into(),
                ));
            }
            Ok(json!({"message":"Simulation complete. No hardware or disk was modified."}))
        }
        Action::ListMenuPorts => {
            if state.session.lock().unwrap().mock_disconnected { return Ok(json!([])); }
            Ok(json!([{"port":"sim:bl616","identity":"[\"sim:bl616\",13467,24928,\"SIMULATION\",\"DEMO\"]","description":"BL616 UPDATE · simulated"}]))
        },
        Action::ListRoms { .. } => Ok(json!([])),
        _ => Err(invalid(
            "This action requires real mode. Browser preview only simulates setup operations.",
        )),
    }
}
struct BusyGuard<'a>(&'a AtomicBool);
impl Drop for BusyGuard<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::SeqCst);
    }
}
#[tauri::command]
async fn snapshot(app: tauri::AppHandle) -> Result<Snapshot> {
    tauri::async_runtime::spawn_blocking(move || snapshot_sync(&app.state::<State>()))
        .await
        .map_err(|e| invalid(e.to_string()))?
}
#[tauri::command]
async fn perform(app: tauri::AppHandle, action: FlasherAction) -> Result<Value> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<State>();
        if state
            .busy
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            return Err(Error::Busy);
        }
        let _guard = BusyGuard(&state.busy);
        state.cancel.store(false, Ordering::SeqCst);
        event(&app, "Operation started", "info");
        let result = execute(&state, action.into(), &app);
        match &result {
            Ok(v) => event(
                &app,
                v.get("message")
                    .and_then(Value::as_str)
                    .unwrap_or("Operation completed"),
                "success",
            ),
            Err(e) => event(&app, e.to_string(), "error"),
        };
        result
    })
    .await
    .map_err(|e| invalid(e.to_string()))?
}
#[tauri::command]
fn cancel(state: tauri::State<State>) {
    state.cancel.store(true, Ordering::SeqCst);
}
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let data = app.path().app_data_dir()?;
            fs::create_dir_all(&data)?;
            let mut saved:Saved = fs::read(data.join("settings.json"))
                .ok()
                .and_then(|b| serde_json::from_slice(&b).ok())
                .unwrap_or_default();
            if saved.repository.is_empty() { saved.repository=REPOSITORY.into(); }
            let resources = app.path().resource_dir()?.join("resources");
            let state = State {
                session: Mutex::new(Session {
                    saved,
                    ..Default::default()
                }),
                busy: AtomicBool::new(false),
                cancel: AtomicBool::new(false),
                simulation: std::env::var("NANOBLAST_SIMULATION").as_deref() == Ok("1"),
                data,
                resources,
            };
            let bundled = state.resources.join("firmware/firmware.json");
            if bundled.is_file() && !state.simulation {
                if let Err(e) = select_firmware(&state, &bundled) {
                    event(
                        app.handle(),
                        format!("Bundled firmware rejected: {e}"),
                        "error",
                    );
                }
            }
            let profile=state.resources.join("nanoblast-v0.2.0");
            match NanoRelease::load(&profile.join("manifest.json")) {
                Ok(release)=>{
                    let sd:SdManifest=read_json(&profile.join("sd.json"))?;sd.validate()?;
                    let mut session=state.session.lock().unwrap();session.nano=Some((release,profile.clone()));session.sd=Some((sd,profile));
                },
                Err(e)=>event(app.handle(),format!("NanoBlast bundled release unavailable: {e}"),"warning"),
            }
            app.manage(state);
            Ok(())
        })
        .on_window_event(|window, window_event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = window_event {
                if window.state::<State>().busy.load(Ordering::SeqCst) {
                    api.prevent_close();
                    event(window.app_handle(), "An update is still running. Keep the app open until verification finishes.", "warning");
                }
            }
        })
        .invoke_handler(tauri::generate_handler![snapshot, perform, cancel])
        .run(tauri::generate_context!())
        .expect("Error running NANOBLAST Flasher");
}

#[cfg(test)]
mod flasher_tests {
    use super::FlasherAction;
    #[test]
    fn desktop_commands_only_accept_firmware_workflows() {
        for request in [
            serde_json::json!({"type":"usb_rom_ports"}),
            serde_json::json!({"type":"usb_rom_upload","port":"x","identity":"x","directory":"nes","files":["x.nes"]}),
            serde_json::json!({"type":"format","device_id":"x","confirmation":"ERASE x","expected_identity":"x","expected_capacity":1}),
        ] { assert!(serde_json::from_value::<FlasherAction>(request).is_err()); }
        assert!(serde_json::from_str::<FlasherAction>(r#"{"type":"list_menu_ports"}"#).is_ok());
    }
}
