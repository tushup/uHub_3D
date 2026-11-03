import { useEffect, useRef } from "react";

import type { AssetType } from "../api";

/** Window event a dialog listens to for the Ctrl/Cmd+Enter save shortcut. */
export const SAVE_DIALOG_EVENT = "uhub:save-dialog";

export interface ShortcutHandlers {
  onNewAsset: (type: AssetType) => void;
  onOpenNewPopover: () => void;
  onFocusSearch: () => void;
  onPreferences: () => void;
  onTab: (index: number) => void;
  onSaveDialog: () => void;
  onEscape: () => void;
}

export function isMacPlatform(): boolean {
  if (typeof navigator === "undefined") return false;
  return /Mac|iPhone|iPad/.test(navigator.platform || navigator.userAgent || "");
}

function isEditableTarget(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) return false;
  return target.closest("input, textarea, select, [contenteditable]") !== null;
}

/**
 * Global shortcuts (STUDIO_DARK §5). The modifier is Cmd on macOS and
 * Ctrl elsewhere. While focus is in an input, only Esc and
 * Ctrl/Cmd+Enter fire.
 */
export function useShortcuts(handlers: ShortcutHandlers, isMac: boolean = isMacPlatform()): void {
  const ref = useRef(handlers);
  ref.current = handlers;

  useEffect(() => {
    function onKeyDown(e: KeyboardEvent) {
      const h = ref.current;
      const mod = isMac ? e.metaKey : e.ctrlKey;
      const inInput = isEditableTarget(e.target);

      if (e.key === "Escape") {
        h.onEscape();
        return;
      }
      if (mod && e.key === "Enter") {
        e.preventDefault();
        h.onSaveDialog();
        return;
      }
      if (inInput) return;

      if (mod && (e.key === "k" || e.key === "K")) {
        e.preventDefault();
        h.onFocusSearch();
      } else if (mod && e.key === ",") {
        e.preventDefault();
        h.onPreferences();
      } else if (mod && (e.key === "n" || e.key === "N")) {
        e.preventDefault();
        if (e.shiftKey) h.onOpenNewPopover();
        else h.onNewAsset("model");
      } else if (!e.altKey && (e.key === "1" || e.key === "2" || e.key === "3")) {
        h.onTab(Number(e.key) - 1);
      }
    }
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [isMac]);
}
