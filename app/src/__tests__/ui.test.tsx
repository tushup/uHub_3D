import { fireEvent, render, screen } from "@testing-library/react";
import { expect, test, vi } from "vitest";

import Button from "../components/ui/Button";
import Chip from "../components/ui/Chip";
import Corners from "../components/ui/Corners";
import Eyebrow from "../components/ui/Eyebrow";
import IconButton from "../components/ui/IconButton";
import Kbd from "../components/ui/Kbd";
import WireCube from "../components/ui/WireCube";

test("Button variant changes classes and click fires", () => {
  const onClick = vi.fn();
  const { rerender } = render(
    <Button variant="primary" onClick={onClick}>
      Save
    </Button>,
  );
  const btn = screen.getByRole("button", { name: "Save" });
  expect(btn.className).toContain("bg-accent");
  expect(btn.className).toContain("active:scale-[.97]");
  fireEvent.click(btn);
  expect(onClick).toHaveBeenCalledTimes(1);
  rerender(<Button variant="danger">Delete</Button>);
  expect(screen.getByRole("button", { name: "Delete" }).className).toContain("text-danger-text");
});

test("IconButton renders its required label as aria-label", () => {
  render(
    <IconButton label="Preferences">
      <span>gear</span>
    </IconButton>,
  );
  expect(screen.getByRole("button", { name: "Preferences" })).toBeTruthy();
});

// @ts-expect-error IconButton without a label must fail type-check
const _missingLabel = <IconButton>gear</IconButton>;
void _missingLabel;

test("Chip filled vs outline", () => {
  const { rerender } = render(<Chip label="BC" filled />);
  expect(screen.getByText("BC").className).toContain("bg-accent");
  rerender(<Chip label="BC" />);
  expect(screen.getByText("BC").className).toContain("border-accent-chip");
});

test("Eyebrow shows an accent section index", () => {
  render(<Eyebrow index="01">Name</Eyebrow>);
  expect(screen.getByText("01").className).toContain("text-accent");
  expect(screen.getByText(/Name/)).toBeTruthy();
});

test("Corners renders four corner marks at the inset", () => {
  const { container } = render(
    <div className="relative">
      <Corners inset={14} />
    </div>,
  );
  const corners = container.querySelectorAll("[data-corner]");
  expect(corners.length).toBe(4);
  expect((corners[0] as HTMLElement).style.top).toBe("14px");
});

test("WireCube renders six faces", () => {
  const { container } = render(<WireCube size={80} />);
  expect(container.querySelectorAll("[data-face]").length).toBe(6);
});

test("Kbd renders its hint", () => {
  render(<Kbd>Ctrl K</Kbd>);
  expect(screen.getByText("Ctrl K").tagName).toBe("KBD");
});
