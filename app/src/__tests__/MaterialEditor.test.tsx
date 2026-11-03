import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { useReducer } from "react";
import { expect, test, vi } from "vitest";

import MaterialEditor from "../components/newasset/MaterialEditor";
import { draftReducer, initDraft, type DraftState } from "../components/newasset/draft";

vi.mock("@tauri-apps/plugin-dialog", () => ({
  open: vi.fn(() => Promise.resolve("C:/t/wood.PNG")),
}));

import { open } from "@tauri-apps/plugin-dialog";

function modelDraft(): DraftState {
  let s = initDraft("model");
  s = draftReducer(s, {
    type: "setModel",
    path: "/m/cube.fbx",
    parsed: {
      format: "fbx",
      meshes: 1,
      vertices: 8,
      materials: [{ name: "Red", textures: [["BaseColor", "/tmp/red.png"]] }],
    },
  });
  return s;
}

function Harness({ initial }: { initial: DraftState }) {
  const [s, d] = useReducer(draftReducer, initial);
  return (
    <div>
      <MaterialEditor draft={s} dispatch={d} />
      <output data-testid="state">{JSON.stringify(s.materials)}</output>
    </div>
  );
}

test("double-click renames material, Enter commits, Esc reverts", () => {
  render(<Harness initial={modelDraft()} />);
  fireEvent.doubleClick(screen.getByText("Red"));
  const input = screen.getByLabelText("Material name") as HTMLInputElement;
  fireEvent.change(input, { target: { value: "Seat" } });
  fireEvent.keyDown(input, { key: "Enter" });
  expect(screen.getByText("Seat")).toBeTruthy();

  fireEvent.doubleClick(screen.getByText("Seat"));
  const input2 = screen.getByLabelText("Material name") as HTMLInputElement;
  fireEvent.change(input2, { target: { value: "Nope" } });
  fireEvent.keyDown(input2, { key: "Escape" });
  expect(screen.getByText("Seat")).toBeTruthy();
  expect(screen.queryByText("Nope")).toBeNull();
});

test("add attribute list excludes existing kinds", () => {
  render(<Harness initial={modelDraft()} />);
  const select = screen.getByLabelText("Add attribute for Red") as HTMLSelectElement;
  const options = Array.from(select.options).map((o) => o.value);
  expect(options).not.toContain("BaseColor");
  expect(options).toContain("Roughness");
  expect(options).toContain("Normal");
});

test("normal row shows OpenGL/DirectX toggle, roughness row does not", () => {
  let s = modelDraft();
  s = draftReducer(s, { type: "addAttribute", index: 0, kind: "Normal" });
  s = draftReducer(s, { type: "addAttribute", index: 0, kind: "Roughness" });
  render(<Harness initial={s} />);
  expect(screen.getByRole("button", { name: "OpenGL" })).toBeTruthy();
  expect(screen.getByRole("button", { name: "DirectX" })).toBeTruthy();
  expect(screen.getAllByLabelText(/Select texture for/)).toHaveLength(3);
});

test("collapsing a material card keeps its attributes", () => {
  let s = modelDraft();
  s = draftReducer(s, { type: "addAttribute", index: 0, kind: "Normal" });
  render(<Harness initial={s} />);
  expect(screen.getByLabelText("Select texture for Normal")).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "Collapse Red" }));
  expect(screen.queryByLabelText("Select texture for Normal")).toBeNull();
  const state = screen.getByTestId("state").textContent ?? "";
  expect(state).toContain("Normal");
  expect(state).toContain("/tmp/red.png");
  fireEvent.click(screen.getByRole("button", { name: "Expand Red" }));
  expect(screen.getByLabelText("Select texture for Normal")).toBeTruthy();
});

test("texture pick sets textureSrc and shows file name", async () => {
  vi.mocked(open).mockResolvedValue("C:/t/wood.PNG");
  let s = modelDraft();
  s = draftReducer(s, { type: "addAttribute", index: 0, kind: "Roughness" });
  render(<Harness initial={s} />);
  fireEvent.click(screen.getByLabelText("Select texture for Roughness"));
  await waitFor(() => expect(screen.getByText("wood.PNG")).toBeTruthy());
});
