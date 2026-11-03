import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, expect, test, vi } from "vitest";

import type { ConnectionInfo } from "../api";
import ExportPanel from "../components/ExportPanel";
import { useStore } from "../store";

vi.mock("../api", async (importOriginal) => {
  const orig = await importOriginal<typeof import("../api")>();
  return {
    ...orig,
    exportAsset: vi.fn(() =>
      Promise.resolve({ jobId: "job-1", ok: true, message: "Imported Chair" }),
    ),
  };
});

import * as api from "../api";

const blender: ConnectionInfo = {
  id: 1,
  dcc: "blender",
  dccVersion: "5.1.2",
  pid: 12345,
  renderers: ["cycles", "eevee"],
};
const maya: ConnectionInfo = {
  id: 2,
  dcc: "maya",
  dccVersion: "2024.2",
  pid: 777,
  renderers: ["arnold"],
};

beforeEach(() => {
  vi.clearAllMocks();
  vi.mocked(api.exportAsset).mockResolvedValue({
    jobId: "job-1",
    ok: true,
    message: "Imported Chair",
  });
  useStore.setState({
    connections: [],
    bridgeStatus: { state: "online", port: 47913 },
    lastExport: {},
    toasts: [],
  });
});

test("export disabled with no connections and shows hint", () => {
  render(<ExportPanel dir="/lib/3D/Furniture/Chair" />);
  expect(
    screen.getByText("Open Blender, Maya or Unreal with the uHub3D plugin enabled."),
  ).toBeTruthy();
  expect(screen.getByRole("button", { name: /^Export/ })).toHaveProperty("disabled", true);
});

test("renderer options follow selected DCC", () => {
  useStore.setState({ connections: [blender, maya] });
  render(<ExportPanel dir="/lib/3D/Furniture/Chair" />);
  fireEvent.click(screen.getByRole("radio", { name: /Blender 5\.1/ }));
  expect(screen.getByRole("radio", { name: "Cycles" })).toBeTruthy();
  expect(screen.getByRole("radio", { name: "EEVEE" })).toBeTruthy();
  fireEvent.click(screen.getByRole("radio", { name: /Maya 2024/ }));
  expect(screen.getByRole("radio", { name: "Arnold" })).toBeTruthy();
  expect(screen.queryByRole("radio", { name: "Cycles" })).toBeNull();
});

test("export calls api with conn id, renderer, dir and toasts result", async () => {
  useStore.setState({ connections: [blender] });
  render(<ExportPanel dir="/lib/3D/Furniture/Chair" />);
  fireEvent.click(screen.getByRole("radio", { name: /Blender 5\.1/ }));
  fireEvent.click(screen.getByRole("radio", { name: "Cycles" }));
  fireEvent.click(screen.getByRole("button", { name: /^Export/ }));
  await waitFor(() => {
    expect(api.exportAsset).toHaveBeenCalledWith(1, "cycles", "/lib/3D/Furniture/Chair");
  });
  await waitFor(() => {
    expect(useStore.getState().toasts.some((t) => t.message === "Imported Chair")).toBe(true);
  });
  expect(useStore.getState().lastExport).toEqual({ connId: 1, renderer: "cycles" });
});

test("export_dock_shows_offline_bridge", () => {
  useStore.setState({
    connections: [blender],
    bridgeStatus: { state: "offline", port: 47913, reason: "Port 47913 in use" },
  });
  render(<ExportPanel dir="/lib/3D/Furniture/Chair" />);
  expect(screen.getByText("Bridge offline: port 47913 in use")).toBeTruthy();
  expect(screen.getByRole("button", { name: /^Export/ })).toHaveProperty("disabled", true);
});

test("ghost_pills_for_unconnected_dccs", () => {
  useStore.setState({ connections: [blender] });
  const { container } = render(<ExportPanel dir="/lib/3D/Furniture/Chair" />);
  expect(screen.getByRole("radio", { name: /Blender 5\.1/ })).toBeTruthy();
  // Unconnected DCCs stay as non-clickable ghost hints at 45% opacity.
  const ghosts = Array.from(container.querySelectorAll(".opacity-45")).map(
    (el) => el.textContent,
  );
  expect(ghosts.join(" ")).toContain("Maya");
  expect(ghosts.join(" ")).toContain("Unreal");
  expect(screen.queryByRole("radio", { name: /Maya/ })).toBeNull();
  expect(screen.queryByRole("radio", { name: /Unreal/ })).toBeNull();
});
