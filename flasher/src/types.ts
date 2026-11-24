export interface FirmwareManifest {
  version: string;
  board: string;
  bitstream: string;
  download_url: string | null;
  sha256: string;
  release_date: string;
  sd_asset_version: string;
}
export interface Device {
  id: string;
  serial: string | null;
  description: string;
  accessible: boolean;
  identification: string;
}
export interface Storage {
  id: string;
  name: string;
  mount: string | null;
  capacity: number;
  available: number;
  filesystem: string;
  removable: boolean;
  system: boolean;
  identity: string;
}
export interface SdManifest {
  version: string;
  firmware_versions: string[];
  directories: string[];
  files: {
    source: string;
    destination: string;
    sha256: string;
    required: boolean;
  }[];
  cores: { name: string; directory: string; extensions: string[] }[];
}
export interface NanoRelease {
  version: string;
  status: string;
  firmware: { bytes: number; sha256: string };
  assets: {
    core_id: number;
    name: string;
    file: string;
    bytes: number;
    offset: number;
    sha256: string;
  }[];
  core_format: string;
  source: string;
  experimental_cores: string[];
}
export interface SelectedRelease {
  kind?: "arcade" | "console";
  video_images?: number;
  hardware_tested?: boolean;
  version: string;
  path: string;
  source: string;
  bytes: number;
  sha256: string;
  cores: number;
  layout: string;
}
export interface Snapshot {
  simulation: boolean;
  nanoblast: NanoRelease | null;
  setup_firmware?: SelectedRelease | null;
  devices: Device[];
  storage: Storage[];
  loader: { path: string; version: string; bundled: boolean } | null;
  warnings: string[];
  firmware: { manifest: FirmwareManifest; path: string } | null;
  sd: SdManifest | null;
  repository: string;
  history: {
    version: string;
    device: string;
    mode: string;
    timestamp: number;
  }[];
  busy: boolean;
}
export interface OperationEvent {
  message: string;
  kind: "info" | "success" | "warning" | "error";
}
export interface Release {
  tag_name: string;
  name: string | null;
  body: string | null;
  published_at: string | null;
  prerelease: boolean;
  assets: { name: string; browser_download_url: string }[];
}
export interface MenuPort {
  port: string;
  identity: string;
  description: string;
}
export type Action =
  | { type: "select_setup_release"; path: string }
  | { type: "install_setup"; port: string; identity: string; confirmation: string }
  | { type: "list_menu_ports" }
  | { type: "simulate"; scenario: string };
export function errorMessage(error: unknown): string {
  if (error instanceof Error) return error.message;
  if (typeof error === "object" && error && "message" in error)
    return String(error.message);
  return String(error);
}
