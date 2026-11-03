import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, expect, test, vi } from "vitest";

import * as api from "../api";
import PreferencesDialog from "../components/PreferencesDialog";
import { useStore } from "../store";

let prefsCb: (() => void) | null = null;

vi.mock("../api", async (importOriginal) => {
  const orig = await importOriginal<typeof import("../api")>();
  return {
    ...orig,
    onPreferences: vi.fn((cb: () => void) => {
      prefsCb = cb;
      return Promise.resolve(() => {});
    }),
    getSettings: vi.fn(() => Promise.resolve({ libraryRoot: "C:/Lib", port: 47913 })),
    setSettings: vi.fn(() => Promise.resolve()),
    listAssets: vi.fn(() => Promise.resolve([])),
    cliPath: vi.fn(() => Promise.resolve("C:/Apps/uhub.exe")),
  };
});

vi.mock("@tauri-apps/plugin-dialog", () => ({
  open: vi.fn(() => Promise.resolve("D:/OtherLib")),
}));

beforeEach(() => {
  vi.clearAllMocks();
  prefsCb = null;
  useStore.setState({ assets: [], toasts: [] });
});

async function openPrefs() {
  render(<PreferencesDialog />);
  if (!prefsCb) throw new Error("onPreferences not subscribed");
  const cb: () => void = prefsCb;
  act(() => cb());
  await screen.findByLabelText("Library Root");
}

test("port out of range disables save", async () => {
  await openPrefs();
  const port = screen.getByLabelText("Bridge Port");
  fireEvent.change(port, { target: { value: "80" } });
  expect((screen.getByRole("button", { name: "Save" }) as HTMLButtonElement).disabled).toBe(true);
  fireEvent.change(port, { target: { value: "47913" } });
  expect((screen.getByRole("button", { name: "Save" }) as HTMLButtonElement).disabled).toBe(false);
});

test("preferences shows cli path with copy", async () => {
  const writeText = vi.fn(() => Promise.resolve());
  Object.defineProperty(navigator, "clipboard", {
    value: { writeText },
    configurable: true,
  });
  await openPrefs();
  fireEvent.click(screen.getByRole("button", { name: "About" }));
  await screen.findByText("C:/Apps/uhub.exe");
  fireEvent.click(screen.getByRole("button", { name: "Copy" }));
  expect(writeText).toHaveBeenCalledWith("C:/Apps/uhub.exe");
  await screen.findByRole("button", { name: "Copied" });
});

test("save calls setSettings and reloads assets", async () => {
  await openPrefs();
  fireEvent.click(screen.getByRole("button", { name: "Save" }));
  await waitFor(() => expect(api.setSettings).toHaveBeenCalledWith({ libraryRoot: "C:/Lib", port: 47913 }));
  await waitFor(() => expect(api.listAssets).toHaveBeenCalled());
  await waitFor(() => expect(screen.queryByLabelText("Library Root")).toBeNull());
});
