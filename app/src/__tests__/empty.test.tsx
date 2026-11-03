import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, expect, test } from "vitest";

import EmptyState from "../components/EmptyState";
import Toasts from "../components/Toasts";
import { useStore } from "../store";

beforeEach(() => {
  useStore.setState({ newAssetRequest: null, toasts: [] });
});

test("each create button opens the matching dialog type", () => {
  render(<EmptyState />);
  fireEvent.click(screen.getByRole("button", { name: /3D Asset/ }));
  expect(useStore.getState().newAssetRequest?.type).toBe("model");
  fireEvent.click(screen.getByRole("button", { name: "Material" }));
  expect(useStore.getState().newAssetRequest?.type).toBe("material");
  fireEvent.click(screen.getByRole("button", { name: "HDRI" }));
  expect(useStore.getState().newAssetRequest?.type).toBe("hdri");
});

test("success toast shows sub and drain bar; error toast stays", () => {
  useStore.setState({
    toasts: [
      { id: 1, kind: "success", message: "Saved Old_Chair", sub: "3D / Furniture · 6 files" },
      { id: 2, kind: "error", message: "An asset named Old_Chair already exists in Furniture" },
    ],
  });
  const { container } = render(<Toasts />);
  expect(screen.getByText("Saved Old_Chair")).toBeTruthy();
  expect(screen.getByText("3D / Furniture · 6 files")).toBeTruthy();
  expect(container.querySelector(".drain-bar")).toBeTruthy();
  expect(screen.getByText(/already exists in Furniture/)).toBeTruthy();
  fireEvent.click(screen.getAllByRole("button", { name: "Dismiss" })[0]);
  expect(screen.queryByText("Saved Old_Chair")).toBeNull();
});
