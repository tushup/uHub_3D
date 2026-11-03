import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { useReducer } from "react";
import { expect, test, vi } from "vitest";

import HdriFields, { HdriDefaults, HdriPreview } from "../components/newasset/HdriFields";
import { draftReducer, initDraft } from "../components/newasset/draft";

vi.mock("../api", async (importOriginal) => {
  const orig = await importOriginal<typeof import("../api")>();
  return {
    ...orig,
    hdrPreview: vi.fn(() =>
      Promise.resolve({ width: 4096, height: 2048, dataUrl: "data:image/png;base64,AAA" }),
    ),
  };
});

vi.mock("@tauri-apps/plugin-dialog", () => ({
  open: vi.fn(() => Promise.resolve("C:/sky.hdr")),
}));

function Harness() {
  const [s, d] = useReducer(draftReducer, undefined, () => initDraft("hdri"));
  return (
    <div>
      <HdriFields draft={s} dispatch={d} />
      <HdriDefaults draft={s} dispatch={d} />
      <output data-testid="src">{s.hdriSrc ?? ""}</output>
      <output data-testid="preview">{s.thumbnailPreview ?? ""}</output>
      <output data-testid="rotation">{s.rotation}</output>
    </div>
  );
}

test("rotation dial: arrows step 1 degree, Shift steps 15, wraps at 360", () => {
  render(<Harness />);
  const dial = screen.getByRole("slider", { name: "Rotation" });
  fireEvent.keyDown(dial, { key: "ArrowRight" });
  expect(screen.getByTestId("rotation").textContent).toBe("1");
  fireEvent.keyDown(dial, { key: "ArrowRight", shiftKey: true });
  expect(screen.getByTestId("rotation").textContent).toBe("16");
  fireEvent.keyDown(dial, { key: "ArrowLeft" });
  expect(screen.getByTestId("rotation").textContent).toBe("15");
  for (let i = 0; i < 23; i++) fireEvent.keyDown(dial, { key: "ArrowRight", shiftKey: true });
  // 15 + 23*15 = 360 -> wraps to 0
  expect(screen.getByTestId("rotation").textContent).toBe("0");
  expect(dial.getAttribute("aria-valuenow")).toBe("0");
});

test("rotation readout wraps at 360 and never shows 360", () => {
  const draft = { ...initDraft("hdri"), rotation: 359.6 };
  render(<HdriDefaults draft={draft} dispatch={vi.fn()} />);
  const dial = screen.getByRole("slider", { name: "Rotation" });
  expect(dial.getAttribute("aria-valuenow")).toBe("0");
  expect(screen.queryByText("360")).toBeNull();
});

test("hdri_preview_has_no_probe_spheres", () => {
  render(<HdriPreview preview="data:image/png;base64,AAA" />);
  const slot = screen.getByTestId("hdri-preview");
  // The old design drew two CSS "probe spheres" (rounded-full
  // gradient discs) under the panorama band.
  expect(slot.querySelectorAll('[class*="rounded-full"]')).toHaveLength(0);
  expect(slot.textContent).not.toMatch(/probe/i);
});

test("hdri_preview_is_square_with_contained_panorama", () => {
  render(<HdriPreview preview="data:image/png;base64,AAA" />);
  const slot = screen.getByTestId("hdri-preview");
  expect(slot.className).toContain("aspect-square");
  const imgs = Array.from(slot.querySelectorAll("img"));
  expect(imgs).toHaveLength(2);
  const back = imgs.find((i) => i.getAttribute("aria-hidden") === "true");
  const front = imgs.find((i) => i.getAttribute("aria-hidden") !== "true");
  expect(back?.className).toContain("object-cover");
  expect(back?.className).toContain("blur-");
  expect(back?.className).toContain("brightness-");
  expect(front?.className).toContain("object-contain");
  expect(front?.className).toContain("w-full");
  expect(screen.getByText("AUTO PREVIEW · TONEMAPPED")).toBeTruthy();
});

test("hdri select shows resolution and preview", async () => {
  render(<Harness />);
  fireEvent.click(screen.getByRole("button", { name: "Select HDRI…" }));
  await waitFor(() => expect(screen.getByText(/4096 × 2048/)).toBeTruthy());
  expect(screen.getByTestId("src").textContent).toBe("C:/sky.hdr");
  expect(screen.getByTestId("preview").textContent).toBe("data:image/png;base64,AAA");
});
