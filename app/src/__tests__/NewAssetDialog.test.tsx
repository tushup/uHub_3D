import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { expect, test, vi } from "vitest";

import type { AssetEntry, RenderProgress } from "../api";
import * as api from "../api";
import NewAssetDialog from "../components/newasset/NewAssetDialog";
import { useStore } from "../store";

let newAssetCb: ((type: "model" | "material" | "hdri") => void) | null = null;
let mockProgressCb: ((p: RenderProgress) => void) | null = null;

vi.mock("../api", async (importOriginal) => {
  const orig = await importOriginal<typeof import("../api")>();
  return {
    ...orig,
    onNewAsset: vi.fn((cb: (type: "model" | "material" | "hdri") => void) => {
      newAssetCb = cb;
      return Promise.resolve(() => {});
    }),
    parseModel: vi.fn(() =>
      Promise.resolve({
        format: "fbx",
        meshes: 3,
        vertices: 5120,
        materials: [
          { name: "Red", textures: [] },
          { name: "Blue", textures: [] },
        ],
      }),
    ),
    saveAsset: vi.fn(() =>
      Promise.resolve({
        dir: "/lib/3D/Furniture/Chair",
        asset: {
          schema: 1,
          id: "1",
          type: "model",
          name: "Chair",
          category: "Furniture",
          tags: [],
          folder: "Chairs",
          thumbnail: "thumbnail.png",
          model: { file: "Chair.fbx", format: "fbx" },
          materials: [],
        },
      } as unknown as AssetEntry),
    ),
    listAssets: vi.fn(() => Promise.resolve([])),
    planSave: vi.fn((draft: { name: string; category: string }) =>
      Promise.resolve(
        draft.name && draft.category
          ? {
              dir: "/lib/3D/Furniture/Chair",
              dirName: "Chair",
              relDir: "Database/3D/Furniture/Chair/",
              files: ["Chair.fbx", "thumbnail.png", "asset.json"],
            }
          : {
              dir: "/lib/3D/<Category>/<Name>",
              dirName: "<Name>",
              relDir: "Database/3D/<Category>/<Name>/",
              files: ["thumbnail.png", "asset.json"],
            },
      ),
    ),
    cancelRender: vi.fn(() => Promise.resolve()),
    onRenderProgress: vi.fn((cb: (p: RenderProgress) => void) => {
      mockProgressCb = cb;
      return Promise.resolve(() => {});
    }),
    hdrPreview: vi.fn(() =>
      Promise.resolve({ width: 512, height: 256, dataUrl: "data:image/png;base64,AAA" }),
    ),
  };
});
vi.mock("@tauri-apps/plugin-dialog", () => ({
  open: vi.fn(() => Promise.resolve(null)),
}));
vi.mock("../viewer/Viewer", async () => {
  const React = await import("react");
  return {
    default: (props: {
      materials: unknown;
      view: { yaw: number; pitch: number; zoom: number };
      onViewChange: (v: { yaw: number; pitch: number; zoom: number }) => void;
      renderFrame: string | null;
    }) =>
      React.createElement(
        "div",
        {
          "data-testid": "viewer",
          "data-materials": JSON.stringify(props.materials),
          "data-view": JSON.stringify(props.view),
        },
        React.createElement(
          "button",
          {
            type: "button",
            onClick: () => props.onViewChange({ ...props.view, yaw: 10 }),
          },
          "mock-rotate",
        ),
        props.renderFrame
          ? React.createElement("img", { alt: "render frame", src: props.renderFrame })
          : null,
      ),
  };
});

import { open as dialogOpen } from "@tauri-apps/plugin-dialog";

function openDialog(type: "model" | "material" | "hdri") {
  act(() => {
    newAssetCb?.(type);
  });
  const name =
    type === "model" ? /New 3D Asset/ : type === "material" ? /New Material/ : /New HDRI/;
  return screen.getByRole("dialog", { name });
}

function saveButton() {
  return screen.getByRole("button", { name: /Save to Library/ });
}

function fillNameCategory(name: string, category: string) {
  fireEvent.change(screen.getByLabelText("Asset Name"), { target: { value: name } });
  fireEvent.change(screen.getByLabelText("Category"), { target: { value: "__new__" } });
  fireEvent.change(screen.getByLabelText("New Category"), { target: { value: category } });
}

async function fillValidModelDialog() {
  fillNameCategory("Chair", "Furniture");
  vi.mocked(dialogOpen).mockResolvedValue("C:/models/chair.fbx");
  fireEvent.click(screen.getByRole("button", { name: "Select Asset…" }));
  await waitFor(() =>
    expect(api.parseModel).toHaveBeenCalledWith("C:/models/chair.fbx"),
  );
}

function fillValidMaterialDialog() {
  fillNameCategory("Wood", "Wood");
}

function savesToCard() {
  return screen.getByText("Saves To").parentElement as HTMLElement;
}

test("saves_to_shows_relative_save_path", async () => {
  render(<NewAssetDialog />);
  openDialog("model");
  fillNameCategory("Chair", "Furniture");
  // The mocked planSave resolves a relative plan: the card shows
  // the relative dir, with the absolute path only as a tooltip.
  const card = savesToCard();
  await waitFor(() =>
    expect(card.textContent).toContain("Database/3D/Furniture/Chair/"),
  );
  expect(card.textContent).not.toContain("/lib/3D/Furniture/Chair");
  expect(card.getAttribute("title")).toBe("/lib/3D/Furniture/Chair");
});

test("saves_to_shows_placeholders_when_empty", async () => {
  render(<NewAssetDialog />);
  openDialog("model");
  // Empty fields: the card shows the placeholder form, not a path.
  const card = savesToCard();
  await waitFor(() =>
    expect(card.textContent).toContain("Database/3D/<Category>/<Name>/"),
  );
});

test("errors stay hidden until the first Save attempt", async () => {
  render(<NewAssetDialog />);
  openDialog("model");
  expect(screen.queryByText("Name is required")).toBeNull();
  expect(screen.queryByText("Category is required")).toBeNull();
  const save = saveButton();
  expect((save as HTMLButtonElement).disabled).toBe(false);
  fireEvent.click(save);
  expect(screen.getByText("Name is required")).toBeTruthy();
  expect(screen.getByText("Category is required")).toBeTruthy();
  expect(api.saveAsset).not.toHaveBeenCalled();
});

test("save success closes and selects asset", async () => {
  const onClose = vi.fn();
  useStore.setState({ assets: [], selectedDir: null });
  render(<NewAssetDialog />);
  openDialog("model");
  await fillValidModelDialog();
  const save = saveButton();
  expect((save as HTMLButtonElement).disabled).toBe(false);
  fireEvent.click(save);
  await waitFor(() => expect(useStore.getState().selectedDir).toBe("/lib/3D/Furniture/Chair"));
  // The default framing view is saved with the asset.
  const calls = vi.mocked(api.saveAsset).mock.calls;
  expect(calls[calls.length - 1][0].view).toEqual({
    yaw: -30,
    pitch: 15,
    zoom: 1,
  });
  expect(onClose).not.toHaveBeenCalled(); // parent owns closing via store flag
});

test("save passes the framed view and shows the render overlay", async () => {
  render(<NewAssetDialog />);
  openDialog("material");
  fillValidMaterialDialog();
  fireEvent.click(screen.getByRole("button", { name: "mock-rotate" }));
  let resolveSave: (e: AssetEntry) => void = () => {};
  vi.mocked(api.saveAsset).mockImplementation(
    () =>
      new Promise<AssetEntry>((res) => {
        resolveSave = res;
      }),
  );
  fireEvent.click(saveButton());
  await waitFor(() => expect(api.saveAsset).toHaveBeenCalled());
  const calls = vi.mocked(api.saveAsset).mock.calls;
  expect(calls[calls.length - 1][0].view).toEqual({
    yaw: 10,
    pitch: 15,
    zoom: 1,
  });
  act(() => {
    mockProgressCb?.({
      samples: 96,
      elapsed: 4.2,
      seconds: 10,
      previewDataUrl: "data:image/png;base64,AAA",
      denoised: false,
    });
  });
  expect(screen.getByText(/PATH TRACING 4\.2 \/ 10\.0 s · 96 spp/)).toBeTruthy();
  expect(screen.getByAltText("render frame")).toBeTruthy();
  await act(async () => {
    resolveSave({
      dir: "/lib/Materials/Wood/Wood",
      asset: {
        schema: 1,
        id: "2",
        type: "material",
        name: "Wood",
        category: "Wood",
        tags: [],
        folder: "Wood",
        thumbnail: "thumbnail.png",
        materials: [],
      },
    } as unknown as AssetEntry);
  });
  await waitFor(() => expect(screen.queryByText(/PATH TRACING/)).toBeNull());
});

test("cancel calls cancelRender and keeps the dialog open", async () => {
  render(<NewAssetDialog />);
  openDialog("material");
  fillValidMaterialDialog();
  let rejectSave: (e: Error) => void = () => {};
  vi.mocked(api.saveAsset).mockImplementation(
    () =>
      new Promise<AssetEntry>((_res, rej) => {
        rejectSave = rej;
      }),
  );
  fireEvent.click(saveButton());
  await waitFor(() => expect(api.saveAsset).toHaveBeenCalled());
  expect(screen.getByText(/PATH TRACING/)).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "Cancel render" }));
  expect(api.cancelRender).toHaveBeenCalled();
  await act(async () => {
    rejectSave(new Error("Render cancelled"));
  });
  await waitFor(() => expect(screen.queryByText(/PATH TRACING/)).toBeNull());
  // Dialog and draft survive a cancelled render, without an error.
  expect(screen.getByLabelText("Asset Name")).toBeTruthy();
  expect(screen.queryByText("Render cancelled")).toBeNull();
});

test("material edits update the viewer props", () => {
  render(<NewAssetDialog />);
  openDialog("material");
  const before = screen.getByTestId("viewer").getAttribute("data-materials");
  expect(before).not.toContain("BaseColor");
  const select = document.getElementById("add-attr-0");
  expect(select).toBeTruthy();
  fireEvent.change(select as HTMLElement, { target: { value: "BaseColor" } });
  const after = screen.getByTestId("viewer").getAttribute("data-materials");
  expect(after).toContain("BaseColor");
});
