import { useCallback, useEffect, useRef, useState } from "react";
import {
  ArrowDownToLine,
  CheckCircle2,
  CircuitBoard,
  FileArchive,
  FolderOpen,
  LoaderCircle,
  RefreshCw,
  Terminal,
  X,
} from "lucide-react";
import { api, desktop } from "./api";
import logo from "./assets/tangfpga.png";
import {
  errorMessage,
  type Action,
  type Device,
  type MenuPort,
  type OperationEvent,
  type Snapshot,
  type SelectedRelease,
} from "./types";

function updatePort(port: MenuPort) {
  try {
    const identity = JSON.parse(port.identity);
    return identity[1] === 13467 && identity[2] === 24928;
  } catch {
    return false;
  }
}
function keepSelection(previous: MenuPort | undefined, ports: MenuPort[]) {
  return (
    ports.find((port) => port.identity === previous?.identity) ??
    (ports.length === 1 ? ports[0] : undefined)
  );
}
export default function App() {
  const [snapshot, setSnapshot] = useState<Snapshot>();
  const [updatePorts, setUpdatePorts] = useState<MenuPort[]>([]);
  const [firmwarePort, setFirmwarePort] = useState<MenuPort>();
  const [programmers, setProgrammers] = useState<Device[]>([]);
  const [programmer, setProgrammer] = useState<Device>();
  const [installation, setInstallation] = useState(false);
  const [boardConfirmed, setBoardConfirmed] = useState(false);
  const [completedKind, setCompletedKind] = useState<string>();
  const [scanning, setScanning] = useState(false);
  const [busy, setBusy] = useState(false);
  const busyRef = useRef(false),
    scanRef = useRef(false);
  useEffect(() => { setBoardConfirmed(false); }, [programmer?.id, programmer?.serial, firmwarePort?.identity]);
  const choosingRef = useRef(false);
  const [choosing, setChoosing] = useState(false);
  const [notice, setNotice] = useState("");
  const [outcome, setOutcome] = useState<"idle" | "success" | "error">("idle");
  const [operation, setOperation] = useState("Ready when you are");
  const [logs, setLogs] = useState<(OperationEvent & { time: string })[]>([]);
  const [stage, setStage] = useState("Working");
  const [installed, setInstalled] = useState(false);
  const [backup, setBackup] = useState("");
  const [logPage, setLogPage] = useState(0);
  const append = useCallback((event: OperationEvent) => {
    if (event.message.startsWith("before:")) setStage("Backing up");
    if (event.message.startsWith("Wrote ")) setStage("Installing");
    if (event.message.startsWith("after:")) setStage("Verifying");
    setLogs((old) => [
      ...old.slice(-299),
      { ...event, time: new Date().toLocaleTimeString() },
    ]);
  }, []);
  const scan = useCallback(async (manual = false) => {
    if (busyRef.current || scanRef.current || choosingRef.current) return;
    scanRef.current = true;
    setScanning(true);
    try {
      const value = await api.snapshot();
      setSnapshot(value);
      if (busyRef.current) return;
      if (value.pending_install && value.pending_install.phase !== "fpga_verified") {
        setUpdatePorts([]); setFirmwarePort(undefined);
        const result = await api.perform({type:"list_install_programmers"});
        const devices = (Array.isArray(result) ? result : []) as Device[];
        setProgrammers(devices);
        setProgrammer(old => devices.find(device => device.id === old?.id && device.serial === old?.serial) ?? (devices.length === 1 ? devices[0] : undefined));
        return;
      }
      const updates = await api.perform({ type: "list_menu_ports" });
      if (busyRef.current) return;
      const ports = (Array.isArray(updates) ? updates : []).filter(
        updatePort,
      ) as MenuPort[];
      setUpdatePorts(ports);
      setFirmwarePort((old) => keepSelection(old, ports));
    } catch (error) {
      if (manual) setNotice(errorMessage(error));
    } finally {
      scanRef.current = false;
      setScanning(false);
    }
  }, []);
  useEffect(() => {
    let active = true,
      timer: ReturnType<typeof setTimeout>;
    const poll = async () => {
      if (!active) return;
      await scan();
      if (active) timer = setTimeout(poll, 6000);
    };
    void poll();
    let unlisten: (() => void) | undefined;
    void api.subscribe(append).then((fn) => {
      if (active) unlisten = fn;
      else fn();
    });
    return () => {
      active = false;
      clearTimeout(timer);
      unlisten?.();
    };
  }, [scan, append]);
  const run = async (action: Action, label: string): Promise<unknown> => {
    if (busyRef.current || scanRef.current) return;
    busyRef.current = true;
    setBusy(true);
    setOutcome("idle");
    setStage("Working");
    setOperation(label);
    setNotice("");
    append({ message: label, kind: "info" });
    try {
      const result = await api.perform(action);
      setOutcome("success");
      setOperation(`${label} complete`);
      append({ message: `${label} complete`, kind: "success" });
      return result;
    } catch (error) {
      const message = errorMessage(error);
      setNotice(message);
      setOutcome("error");
      setOperation(`${label} failed`);
      append({ message, kind: "error" });
    } finally {
      busyRef.current = false;
      setBusy(false);
    }
  };
  const locked = busy || scanning || choosing;
  const simulation = snapshot?.simulation ?? !desktop;
  const arcade = snapshot?.setup_firmware?.kind === "arcade";
  const pending = snapshot?.pending_install;
  const needsProgrammer = !!pending && pending.phase !== "fpga_verified";
  const connected = needsProgrammer ? !!programmer : !!firmwarePort;
  const selectRelease = async (folder = false) => {
    if (busyRef.current || scanRef.current || choosingRef.current) return;
    setBoardConfirmed(false);
    choosingRef.current = true;
    setChoosing(true);
    try {
      const paths = simulation
        ? ["SIMULATION-RELEASE"]
        : await api.choose(folder ? "release_folder" : "release");
      if (!paths.length) return;
      setInstalled(false);
      setBackup("");
      setSnapshot((old) => (old ? { ...old, setup_firmware: null } : old));
      const value = await run(
        { type: "select_setup_release", path: paths[0] },
        "Check release files",
      );
      if (value && typeof value === "object")
        setSnapshot((old) =>
          old ? { ...old, setup_firmware: value as SelectedRelease } : old,
        );
    } catch (error) {
      setNotice(errorMessage(error));
    } finally {
      choosingRef.current = false;
      setChoosing(false);
    }
  };
  const flash = async () => {
    if (busyRef.current || scanRef.current || !firmwarePort || !snapshot?.setup_firmware) return;
    setInstalled(false);
    setBackup("");
    const result = await run(
      {
        type: installation ? "prepare_system" : "install_setup",
        port: firmwarePort.port,
        identity: firmwarePort.identity,
        confirmation: installation ? `INSTALL ${arcade ? "ARCADE" : "CONSOLE"} NANO20K ${firmwarePort.port}` : `UPDATE ${firmwarePort.port}`,
      },
      installation ? "Prepare Nano 20K programmer" : arcade ? "Install Arcade application" : "Install firmware and cores",
    );
    if (installation && result) { setBoardConfirmed(false); await scan(); return; }
    if (result && typeof result === "object") {
      setInstalled(true);
      if ("backup" in result) setBackup(String(result.backup));
    }
  };
  const continueInstallation = async () => {
    if (!pending || !boardConfirmed) return;
    if (pending.phase === "fpga_verified") {
      if (!firmwarePort) return;
      const result = await run({type:"finish_system",port:firmwarePort.port,identity:firmwarePort.identity,
        confirmation:`FINISH ${pending.target.toUpperCase()} NANO20K ${firmwarePort.port}`}, "Finish system installation");
      if (result) {
        setInstalled(true); setCompletedKind(pending.target);
        if (typeof result === "object" && "backup" in result) setBackup(String(result.backup));
      }
    } else {
      if (!programmer) return;
      await run({type:"program_system_fpga",device_id:programmer.id,serial:programmer.serial,
        confirmation:`FPGA NANO20K ${programmer.id}`}, "Back up and install FPGA boot image");
    }
    setBoardConfirmed(false); await scan();
  };
  const phase = busy
    ? stage
    : outcome === "success"
      ? "Complete"
      : outcome === "error"
        ? "Needs attention"
        : "Idle";

  return (
    <div className="setup-app flasher-app">
      <header className="retro-header">
        <div className="logo-crop">
          <img className="retro-art" src={logo} alt="TangFPGA" />
        </div>
        <div className="compact-header">
          <div className="compact-brand">
            <span className="pixel-cursor" aria-hidden="true">
              ▶
            </span>{" "}
            FLASHER <span className="app-version">v0.3.1</span>
          </div>
          <button
            className="icon-button"
            aria-label="Find console"
            disabled={locked}
            onClick={() => void scan(true)}
          >
            <RefreshCw size={16} className={scanning ? "spin" : ""} />
          </button>
        </div>
      </header>
      <main>
        {simulation && (
          <div className="preview-strip">
            <span>Preview · no hardware access</span>
            <select
              aria-label="Preview console mode"
              defaultValue="connected"
              disabled={locked}
              onChange={async (event) => {
                await run(
                  { type: "simulate", scenario: event.target.value },
                  "Change preview mode",
                );
                await scan();
              }}
            >
              <option value="connected">UPDATE</option>
              <option value="disconnected">Offline</option>
              <option value="failure">Failure</option>
              <option value="arcade">Arcade</option>
            </select>
          </div>
        )}
        <div
          className={`compact-connection ${connected ? "connected" : ""}`}
        >
          <i />
          {scanning
            ? "Looking for console…"
            : needsProgrammer ? (programmer ? "FPGA programmer connected" : "Reconnect without UPDATE")
            : firmwarePort
              ? "UPDATE mode connected"
              : "No console connected"}
        </div>
        {notice && (
          <div
            role={outcome === "error" ? "alert" : "status"}
            className={`notice ${outcome === "error" ? "error" : ""}`}
          >
            <span>{notice}</span>
            <button
              className="icon-button"
              aria-label="Dismiss message"
              onClick={() => setNotice("")}
            >
              <X size={15} />
            </button>
          </div>
        )}
        <div className="workspace-grid">
          {pending ? <section className="panel compact-firmware">
            <div className="compact-title"><CircuitBoard size={19}/><h1>Install {pending.target === "arcade" ? "Arcade" : "Console"}</h1><span className="mini-tag">Step {pending.phase === "fpga_verified" ? "3" : "2"} / 3</span></div>
            <p className="hardware-scope">{pending.version}<br/>{pending.phase === "fpga_verified"
              ? "Unplug USB. Hold UPDATE while reconnecting the same board, then release UPDATE."
              : "Unplug USB. Reconnect the same board normally, without holding UPDATE. Keep HDMI and the SD card disconnected."}</p>
            <p className="release-warning">{pending.phase === "fpga_verified"
              ? "The FPGA boot image is verified. Finish installing the controller application and system data."
              : "This step backs up the complete FPGA flash, replaces its boot image, then verifies full readback. Connect only the board being installed."}</p>
            {pending.phase !== "fpga_verified" && programmers.length > 1 && <select aria-label="Installation programmer" value={programmer?.id ?? ""} disabled={locked} onChange={event => setProgrammer(programmers.find(device => device.id === event.target.value))}><option value="">Choose programmer</option>{programmers.map(device => <option key={device.id} value={device.id}>{device.description} · {device.id}</option>)}</select>}
            {pending.phase === "fpga_verified" && updatePorts.length > 1 && <select aria-label="Installation UPDATE port" value={firmwarePort?.identity ?? ""} disabled={locked} onChange={event => setFirmwarePort(updatePorts.find(port => port.identity === event.target.value))}><option value="">Choose UPDATE port</option>{updatePorts.map(port => <option key={port.identity} value={port.identity}>{port.port}</option>)}</select>}
            <label className="install-confirm"><input type="checkbox" checked={boardConfirmed} disabled={locked} onChange={event => setBoardConfirmed(event.target.checked)}/>This is the same Tang Nano 20K. Replace its selected system.</label>
            <button className="primary full-width" disabled={locked || !boardConfirmed || (pending.phase === "fpga_verified" ? !firmwarePort : !programmer?.accessible)} onClick={() => void continueInstallation()}>{pending.phase === "fpga_verified" ? "3 · Finish controller installation" : "2 · Install FPGA boot image"}</button>
            <p className="hardware-scope">If no device appears, use Find console above.{pending.phase !== "fpga_verified" && " On Windows, the JTAG interface needs a compatible libusb/WinUSB driver."}</p>
            <details className="backup-path"><summary>Installation backup folder</summary><code>{pending.backup}</code></details>
          </section> : <section
            className="panel compact-firmware"
            aria-labelledby="firmware-title"
          >
            <div className="compact-title">
              <CircuitBoard size={19} />
              <h1 id="firmware-title">{arcade ? "Arcade application" : "Firmware & cores"}</h1>
              <span className="mini-tag">
                {snapshot?.setup_firmware ? "Selected" : "Release file"}
              </span>
            </div>
            <label className="install-mode">Operation<select aria-label="Flashing operation" value={installation ? "install" : "update"} disabled={locked} onChange={event => {setInstallation(event.target.value === "install");setBoardConfirmed(false);setInstalled(false);}}><option value="update">Update existing system</option><option value="install">First install / switch system</option></select></label>
            <p className="hardware-scope">20K · BL616 UPDATE<br />
              {installation ? "Step 1 of 3: back up the controller and install the Nano 20K programmer. Remove the SD card and HDMI first." : arcade
                ? "Application only · existing Arcade installation required. Remove SD card and disconnect HDMI before updating."
                : "Choose a console or Arcade System release."}
            </p>
            <div className="release-picker">
              <button
                className="choose-release"
                disabled={locked}
                onClick={() => void selectRelease()}
              >
                <FileArchive size={15} />
                Choose release…
              </button>
              <button
                className="icon-button folder-picker"
                title="Choose an extracted release folder"
                aria-label="Choose an extracted release folder"
                disabled={locked}
                onClick={() => void selectRelease(true)}
              >
                <FolderOpen size={17} />
              </button>
            </div>
            <p
              className="selected-release"
              title={snapshot?.setup_firmware?.source}
            >
              {snapshot?.setup_firmware ? (
                <>
                  <strong>{snapshot.setup_firmware.version}</strong>
                  <span>{snapshot.setup_firmware.source}</span>
                </>
              ) : (
                "Select a downloaded ZIP or release manifest"
              )}
            </p>
            {arcade && !installation && (
              <p className="release-warning">
                Recovery cache preserved. Copy the release’s SD-card files separately.
                {snapshot?.setup_firmware?.hardware_tested === false && " Hardware validation pending for this release."}
              </p>
            )}
            {updatePorts.length > 1 && (
              <select
                aria-label="Console to update"
                value={firmwarePort?.identity ?? ""}
                disabled={locked}
                onChange={(event) =>
                  setFirmwarePort(
                    updatePorts.find(
                      (port) => port.identity === event.target.value,
                    ),
                  )
                }
              >
                <option value="">Choose console</option>
                {updatePorts.map((port) => (
                  <option key={port.identity} value={port.identity}>
                    {port.port}
                  </option>
                ))}
              </select>
            )}
            {installation && <label className="install-confirm"><input type="checkbox" checked={boardConfirmed} disabled={locked} onChange={event => setBoardConfirmed(event.target.checked)}/>This is a Tang Nano 20K. Back up and replace its current system.</label>}
            <button
              className="primary full-width"
              disabled={
                locked ||
                !firmwarePort ||
                !snapshot?.setup_firmware || (installation && !boardConfirmed)
              }
              onClick={() => void flash()}
            >
              {busy && operation.startsWith("Install") ? (
                <LoaderCircle size={17} className="spin" />
              ) : (
                <ArrowDownToLine size={17} />
              )}
              {installation ? "1 · Prepare Nano 20K" : arcade ? "Flash Arcade application" : "Flash firmware & cores"}
            </button>
            {!firmwarePort && (
              <p className="connection-hint">UPDATE mode required</p>
            )}
            {installed && (
              <div className="success-card" role="status">
                <CheckCircle2 size={18} />
                <div>
                  <strong>
                    {simulation ? "Preview complete" : "Installed & verified"}
                  </strong>
                  <p>
                    {simulation
                      ? "No hardware or files were modified."
                      : arcade || completedKind === "arcade"
                        ? "Unplug all USB and power. Insert the prepared SD card, reconnect HDMI and power on without UPDATE."
                        : "Unplug all USB and power for 5 seconds. Reconnect, then select a core."}
                  </p>
                  {backup && (
                    <details>
                      <summary>Backup</summary>
                      <code>{backup}</code>
                    </details>
                  )}
                </div>
              </div>
            )}
          </section>}

        </div>
        <div className={`compact-status ${outcome}`} role="status">
          {busy ? (
            <LoaderCircle size={15} className="spin" />
          ) : outcome === "success" ? (
            <CheckCircle2 size={15} />
          ) : (
            <Terminal size={15} />
          )}
          <span>{busy ? `${operation} · ${phase}` : operation}</span>
        </div>
        <details className="compact-log">
          <summary>Activity log</summary>
          <div className="log-popover">
            <div className="activity-log" role="log" aria-live="off">
              {logs.length ? (
                logs
                  .slice()
                  .reverse()
                  .slice(logPage * 5, logPage * 5 + 5)
                  .map((log, index) => (
                    <div key={index} className={log.kind}>
                      <time>{log.time}</time>
                      <span title={log.message}>{log.message}</span>
                    </div>
                  ))
              ) : (
                <p>No activity yet.</p>
              )}
            </div>
            {logs.length > 5 && (
              <div className="pagination">
                <button
                  className="text-button"
                  disabled={logPage === 0}
                  onClick={() => setLogPage((page) => page - 1)}
                >
                  Newer
                </button>
                <span>
                  {logPage + 1} / {Math.ceil(logs.length / 5)}
                </span>
                <button
                  className="text-button"
                  disabled={(logPage + 1) * 5 >= logs.length}
                  onClick={() => setLogPage((page) => page + 1)}
                >
                  Older
                </button>
              </div>
            )}
          </div>
        </details>
      </main>
    </div>
  );
}
