import { fireEvent, render } from "@testing-library/react";
import { expect, test, vi } from "vitest";

import { useShortcuts, type ShortcutHandlers } from "../hooks/useShortcuts";

function makeHandlers(): ShortcutHandlers {
  return {
    onNewAsset: vi.fn(),
    onOpenNewPopover: vi.fn(),
    onFocusSearch: vi.fn(),
    onPreferences: vi.fn(),
    onTab: vi.fn(),
    onSaveDialog: vi.fn(),
    onEscape: vi.fn(),
  };
}

function Harness({ handlers, isMac }: { handlers: ShortcutHandlers; isMac: boolean }) {
  useShortcuts(handlers, isMac);
  return <input aria-label="field" />;
}

test("windows: Ctrl+N new 3D, Ctrl+Shift+N popover", () => {
  const h = makeHandlers();
  render(<Harness handlers={h} isMac={false} />);
  fireEvent.keyDown(window, { key: "n", ctrlKey: true });
  expect(h.onNewAsset).toHaveBeenCalledWith("model");
  fireEvent.keyDown(window, { key: "N", ctrlKey: true, shiftKey: true });
  expect(h.onOpenNewPopover).toHaveBeenCalledTimes(1);
  // Meta does nothing on Windows mapping.
  fireEvent.keyDown(window, { key: "n", metaKey: true });
  expect(h.onNewAsset).toHaveBeenCalledTimes(1);
});

test("mac: Cmd maps, Ctrl does not", () => {
  const h = makeHandlers();
  render(<Harness handlers={h} isMac />);
  fireEvent.keyDown(window, { key: "n", metaKey: true });
  expect(h.onNewAsset).toHaveBeenCalledWith("model");
  fireEvent.keyDown(window, { key: "n", ctrlKey: true });
  expect(h.onNewAsset).toHaveBeenCalledTimes(1);
  fireEvent.keyDown(window, { key: ",", metaKey: true });
  expect(h.onPreferences).toHaveBeenCalledTimes(1);
});

test("K focuses search, comma opens preferences, digits switch tabs", () => {
  const h = makeHandlers();
  render(<Harness handlers={h} isMac={false} />);
  fireEvent.keyDown(window, { key: "k", ctrlKey: true });
  expect(h.onFocusSearch).toHaveBeenCalledTimes(1);
  fireEvent.keyDown(window, { key: ",", ctrlKey: true });
  expect(h.onPreferences).toHaveBeenCalledTimes(1);
  fireEvent.keyDown(window, { key: "1" });
  fireEvent.keyDown(window, { key: "2" });
  fireEvent.keyDown(window, { key: "3" });
  expect(h.onTab).toHaveBeenNthCalledWith(1, 0);
  expect(h.onTab).toHaveBeenNthCalledWith(2, 1);
  expect(h.onTab).toHaveBeenNthCalledWith(3, 2);
});

test("nothing fires in an input except Esc and Ctrl+Enter", () => {
  const h = makeHandlers();
  const { getByLabelText } = render(<Harness handlers={h} isMac={false} />);
  const input = getByLabelText("field");
  fireEvent.keyDown(input, { key: "n", ctrlKey: true });
  fireEvent.keyDown(input, { key: "k", ctrlKey: true });
  fireEvent.keyDown(input, { key: "1" });
  fireEvent.keyDown(input, { key: "," , ctrlKey: true });
  expect(h.onNewAsset).not.toHaveBeenCalled();
  expect(h.onFocusSearch).not.toHaveBeenCalled();
  expect(h.onTab).not.toHaveBeenCalled();
  expect(h.onPreferences).not.toHaveBeenCalled();
  fireEvent.keyDown(input, { key: "Enter", ctrlKey: true });
  expect(h.onSaveDialog).toHaveBeenCalledTimes(1);
  fireEvent.keyDown(input, { key: "Escape" });
  expect(h.onEscape).toHaveBeenCalledTimes(1);
});

test("Esc and Ctrl+Enter fire outside inputs too", () => {
  const h = makeHandlers();
  render(<Harness handlers={h} isMac={false} />);
  fireEvent.keyDown(window, { key: "Escape" });
  expect(h.onEscape).toHaveBeenCalledTimes(1);
  fireEvent.keyDown(window, { key: "Enter", ctrlKey: true });
  expect(h.onSaveDialog).toHaveBeenCalledTimes(1);
});
