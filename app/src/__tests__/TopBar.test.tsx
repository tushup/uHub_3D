import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, expect, test } from "vitest";

import TopBar from "../components/TopBar";
import { useStore } from "../store";

beforeEach(() => {
  useStore.setState({ tab: "material", category: null, search: "", selectedDir: null, assets: [] });
});

test("TopBar switches tab on click", () => {
  render(<TopBar />);
  fireEvent.click(screen.getByRole("button", { name: "HDRI" }));
  expect(useStore.getState().tab).toBe("hdri");
});

test("New asset popover requests the matching dialog", () => {
  useStore.setState({ newAssetRequest: null });
  render(<TopBar />);
  fireEvent.click(screen.getByRole("button", { name: /New asset/ }));
  fireEvent.click(screen.getByRole("menuitem", { name: /Material/ }));
  expect(useStore.getState().newAssetRequest?.type).toBe("material");
});

test("TopBar search input updates store search", () => {
  render(<TopBar />);
  fireEvent.change(screen.getByLabelText("Search assets"), { target: { value: "chair" } });
  expect(useStore.getState().search).toBe("chair");
});
