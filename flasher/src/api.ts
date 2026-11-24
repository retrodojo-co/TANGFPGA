import { invoke, isTauri } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import type {
  Action,
  OperationEvent,
  Snapshot,
  SelectedRelease,
} from "./types";
export const desktop = isTauri();
type Listener = (event: OperationEvent) => void;
export class Simulator {
  scenario = "connected";
  private listeners = new Set<Listener>();
  private running = false;
  private selectedRelease: SelectedRelease | null = null;
  private cancelled = false;
  on(listener: Listener) {
    this.listeners.add(listener);
    return () => {
      this.listeners.delete(listener);
    };
  }
  emit(message: string, kind: OperationEvent["kind"] = "info") {
    this.listeners.forEach((listener) => listener({ message, kind }));
  }
  snapshot(): Snapshot {
    return {
      simulation: true,
      nanoblast: null,
      setup_firmware: this.selectedRelease,
      devices: [],
      storage: [],
      loader: null,
      warnings: [],
      firmware: null,
      sd: null,
      repository: "",
      history: [],
      busy: this.running,
    };
  }
  cancel() {
    this.cancelled = true;
  }
  async perform(action: Action): Promise<unknown> {
    if (this.running) throw new Error("Another operation is running.");
    if (action.type === "simulate") {
      this.scenario = action.scenario;
      return null;
    }
    if (action.type === "select_setup_release") {
      if (action.path !== "SIMULATION-RELEASE")
        throw new Error("Preview cannot read actual release files.");
      this.selectedRelease = {
        version: this.scenario === "arcade" ? "Demo Arcade release" : "Demo release",
        kind: this.scenario === "arcade" ? "arcade" : "console",
        video_images: this.scenario === "arcade" ? 9 : undefined,
        hardware_tested: false,
        path: "SIMULATION",
        source: "Demo-release.zip",
        bytes: 667424,
        sha256: "0".repeat(64),
        cores: this.scenario === "arcade" ? 0 : 4,
        layout: this.scenario === "arcade" ? "Arcade V1 / application only" : "BL616 / NBCRXZ01",
      };
      return this.selectedRelease;
    }
    if (action.type === "install_setup" && !this.selectedRelease)
      throw new Error("Choose a downloaded firmware release first.");
    if (action.type === "list_menu_ports")
      return this.scenario === "disconnected" ? [] : [{
        port: "sim:bl616",
        identity: JSON.stringify(["sim:bl616",13467,24928,"SIMULATION","DEMO"]),
        description: "BL616 UPDATE · simulated",
      }];
    if (action.type !== "install_setup")
      throw new Error("TangFPGA Flasher only supports firmware updates.");
    if (this.scenario === "disconnected")
      throw new Error("Simulated programmer is disconnected.");
    if (action.confirmation !== `UPDATE ${action.port}`)
      throw new Error("Exact UPDATE-port confirmation required.");
    this.running = true;
    this.cancelled = false;
    try {
      for (const line of [
        "Validating selected device…",
        "Transferring data…",
        "Verifying installation…",
      ]) {
        this.emit(`Simulation: ${line}`);
        await new Promise((resolve) => setTimeout(resolve, 450));
        if (this.cancelled) throw new Error("Simulation cancelled.");
      }
      if (this.scenario === "failure")
        throw new Error(
          "Simulated programming failure. Check cable and permissions, then retry.",
        );
      const message = "Simulation complete. No hardware or disk was modified.";
      this.emit(message, "success");
      return { message };
    } catch (error) {
      this.emit(
        error instanceof Error ? error.message : String(error),
        "error",
      );
      throw error;
    } finally {
      this.running = false;
    }
  }
}
const simulator = new Simulator();
export const api = {
  snapshot: () =>
    desktop
      ? invoke<Snapshot>("snapshot")
      : Promise.resolve(simulator.snapshot()),
  perform: (action: Action) =>
    desktop
      ? invoke<unknown>("perform", { action })
      : simulator.perform(action),
  cancel: () =>
    desktop ? invoke<void>("cancel") : Promise.resolve(simulator.cancel()),
  subscribe: async (listener: Listener) =>
    desktop
      ? listen<OperationEvent>("operation", (event) => listener(event.payload))
      : simulator.on(listener),
  choose: async (kind: "release" | "release_folder"): Promise<string[]> => {
    if (!desktop)
      throw new Error("File selection is available in the desktop application. This is an isolated preview.");
    const result = await open({
      multiple: false,
      directory: kind === "release_folder",
      filters: kind === "release_folder" ? undefined : [{
        name: "TangFPGA firmware release ZIP or manifest",
        extensions: ["zip", "json"],
      }],
    });
    return result ? (Array.isArray(result) ? result : [result]) : [];
  },
};
