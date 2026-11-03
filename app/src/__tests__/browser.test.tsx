import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, expect, test, vi } from "vitest";

vi.mock("@tauri-apps/api/core", () => ({
  convertFileSrc: (path: string) => `asset://localhost/${path}`,
}));

import type { AssetEntry, AssetType } from "../api";
import AssetGrid from "../components/AssetGrid";
import CategorySidebar from "../components/CategorySidebar";
import { useStore } from "../store";

function entry(type: AssetType, category: string, name: string, tags: string[] = []): AssetEntry {
  return {
    dir: `/lib/${type}/${category}/${name}`,
    asset: {
      schema: 1,
      id: name,
      type,
      name,
      category,
      tags,
      created: "2026-01-01T00:00:00Z",
      thumbnail: null,
      model: type === "model" ? { file: `${name}.fbx`, format: "fbx", meshes: 1, vertices: 8 } : null,
      materials: [],
      hdri: null,
    },
  };
}

const chair = entry("model", "Furniture", "Chair", ["wood"]);
const boulder = entry("model", "Rocks", "Boulder");

beforeEach(() => {
  useStore.setState({
    tab: "model",
    category: null,
    search: "",
    selectedDir: null,
    assets: [chair, boulder],
    connections: [],
    toasts: [],
  });
});

test("clicking category filters grid", () => {
  render(
    <>
      <CategorySidebar />
      <AssetGrid />
    </>,
  );
  expect(screen.getByText("Chair")).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: /Rocks/ }));
  expect(screen.queryByText("Chair")).toBeNull();
  expect(screen.getByText("Boulder")).toBeTruthy();
});

test("card without thumbnail shows placeholder", () => {
  render(<AssetGrid />);
  expect(screen.getAllByTestId("placeholder-cube").length).toBeGreaterThan(0);
});

test("click card selects it", () => {
  render(<AssetGrid />);
  fireEvent.click(screen.getByRole("button", { name: /Chair/ }));
  expect(useStore.getState().selectedDir).toBe(chair.dir);
});

test("empty library message", () => {
  useStore.setState({ assets: [] });
  render(<AssetGrid />);
  expect(screen.getByText(/Library is empty/)).toBeTruthy();
});

test("hdri cards are 2 to 1", () => {
  const skyBase = entry("hdri", "Sky", "Noon");
  const sky: AssetEntry = { ...skyBase, asset: { ...skyBase.asset, thumbnail: "thumbnail.png" } };
  useStore.setState({ tab: "hdri", assets: [sky] });
  render(<AssetGrid />);
  const card = screen.getByRole("button", { name: /Noon/ });
  const media = card.querySelector("span");
  expect(media?.className).toContain("aspect-[2/1]");
});

test("model thumbnail uses contain", () => {
  const withThumb: AssetEntry = {
    ...chair,
    asset: { ...chair.asset, thumbnail: "thumbnail.png" },
  };
  useStore.setState({ assets: [withThumb] });
  const { container } = render(<AssetGrid />);
  const img = container.querySelector("img");
  expect(img).toBeTruthy();
  expect(img?.className).toContain("object-contain");
  expect(img?.className).not.toContain("object-cover");
});
