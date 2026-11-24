import { afterEach, describe, expect, it, vi } from "vitest";
import { Simulator } from "./api";
import type { Action } from "./types";
const update: Action = { type: "install_setup", port: "sim:bl616", identity: "SIMULATION", confirmation: "UPDATE sim:bl616" };
const choose = (sim: Simulator) => sim.perform({ type: "select_setup_release", path: "SIMULATION-RELEASE" });
afterEach(() => vi.useRealTimers());
describe("firmware-only flasher", () => {
  it("starts without an implicit firmware selection", async () => {
    const sim = new Simulator();
    expect(sim.snapshot().simulation).toBe(true);
    expect(sim.snapshot().setup_firmware).toBeNull();
    await expect(sim.perform(update)).rejects.toThrow("Choose a downloaded");
  });
  it("lists only simulated UPDATE ports and handles disconnection", async () => {
    const sim = new Simulator();
    expect(await sim.perform({ type: "list_menu_ports" })).toMatchObject([{ port: "sim:bl616" }]);
    await choose(sim);
    await sim.perform({ type: "simulate", scenario: "disconnected" });
    expect(await sim.perform({ type: "list_menu_ports" })).toEqual([]);
    await expect(sim.perform(update)).rejects.toThrow("disconnected");
  });
  it("distinguishes Arcade application-only releases in preview", async () => {
    const sim = new Simulator();
    await sim.perform({type: "simulate", scenario: "arcade"});
    await choose(sim);
    expect(sim.snapshot().setup_firmware).toMatchObject({kind: "arcade", cores: 0, video_images: 9, hardware_tested: false});
  });
  it("requires exact device confirmation", async () => {
    const sim = new Simulator(); await choose(sim);
    await expect(sim.perform({ ...update, confirmation: "yes" })).rejects.toThrow("confirmation");
  });
  it("never reads real files in the preview", async () => {
    const sim = new Simulator();
    await expect(sim.perform({ type: "select_setup_release", path: "/tmp/real.zip" })).rejects.toThrow("actual release");
  });
  it("rejects removed ROM and disk actions even at runtime", async () => {
    const sim = new Simulator();
    for (const type of ["usb_rom_ports", "usb_rom_upload", "usb_rom_list", "format", "import_roms"])
      await expect(sim.perform({ type } as unknown as Action)).rejects.toThrow("only supports firmware");
  });
  it("serializes updates and releases the lock after verification", async () => {
    vi.useFakeTimers(); const sim = new Simulator(); await choose(sim);
    const task = sim.perform(update);
    await expect(sim.perform(update)).rejects.toThrow("running");
    await vi.runAllTimersAsync();
    await expect(task).resolves.toMatchObject({ message: expect.stringContaining("No hardware") });
    expect(sim.snapshot().busy).toBe(false);
  });
  it("reports failure without claiming success", async () => {
    vi.useFakeTimers(); const sim = new Simulator(); await choose(sim);
    await sim.perform({ type: "simulate", scenario: "failure" });
    const events: string[] = []; sim.on(e => events.push(e.kind));
    const failure = expect(sim.perform(update)).rejects.toThrow("failure");
    await vi.runAllTimersAsync(); await failure;
    expect(events).not.toContain("success"); expect(sim.snapshot().busy).toBe(false);
  });
});
