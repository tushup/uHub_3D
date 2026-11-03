import { useEffect, useRef, useState } from "react";

import type { AssetType } from "../api";
import { isMacPlatform } from "../hooks/useShortcuts";
import { useStore } from "../store";
import IconButton from "./ui/IconButton";
import Kbd from "./ui/Kbd";
import WindowControls from "./WindowControls";

export const TABS: { type: AssetType; label: string; heading: string }[] = [
  { type: "material", label: "Material", heading: "Materials" },
  { type: "model", label: "3D", heading: "3D Assets" },
  { type: "hdri", label: "HDRI", heading: "HDRIs" },
];

const NEW_ITEMS: { type: AssetType; label: string; hint?: string }[] = [
  { type: "model", label: "3D Asset", hint: "N" },
  { type: "material", label: "Material" },
  { type: "hdri", label: "HDRI" },
];

function Logo() {
  return (
    <svg width="22" height="22" viewBox="0 0 24 24" fill="none" stroke="#EDEDEF" strokeWidth="1.6" strokeLinejoin="round" aria-hidden="true">
      <path d="M12 2.8 20 7.4v9.2L12 21.2 4 16.6V7.4z" />
      <path d="M4 7.4 12 12l8-4.6M12 12v9.2" />
      <circle cx="12" cy="12" r="1.8" fill="#D4FF3A" stroke="none" />
    </svg>
  );
}

function SearchIcon() {
  return (
    <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" aria-hidden="true">
      <circle cx="11" cy="11" r="7" />
      <path d="m20 20-3.5-3.5" />
    </svg>
  );
}

function SlidersIcon() {
  return (
    <svg width="17" height="17" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
      <path d="M4 7h10M18 7h2M4 17h4M12 17h8" />
      <circle cx="16" cy="7" r="2" />
      <circle cx="10" cy="17" r="2" />
    </svg>
  );
}

export default function TopBar() {
  const tab = useStore((s) => s.tab);
  const setTab = useStore((s) => s.setTab);
  const search = useStore((s) => s.search);
  const setSearch = useStore((s) => s.setSearch);
  const requestNewAsset = useStore((s) => s.requestNewAsset);
  const requestPreferences = useStore((s) => s.requestPreferences);
  const newPopoverNonce = useStore((s) => s.newPopoverNonce);
  const [popoverOpen, setPopoverOpen] = useState(false);
  const popoverRef = useRef<HTMLDivElement>(null);
  const mac = isMacPlatform();
  const tabIndex = Math.max(0, TABS.findIndex((t) => t.type === tab));

  useEffect(() => {
    if (newPopoverNonce > 0) setPopoverOpen(true);
  }, [newPopoverNonce]);

  useEffect(() => {
    if (!popoverOpen) return;
    function onKey(e: KeyboardEvent) {
      if (e.key === "Escape") setPopoverOpen(false);
    }
    function onPointer(e: PointerEvent) {
      if (popoverRef.current && !popoverRef.current.contains(e.target as Node)) {
        setPopoverOpen(false);
      }
    }
    window.addEventListener("keydown", onKey);
    window.addEventListener("pointerdown", onPointer);
    return () => {
      window.removeEventListener("keydown", onKey);
      window.removeEventListener("pointerdown", onPointer);
    };
  }, [popoverOpen]);

  return (
    <header
      data-tauri-drag-region
      className="flex h-[52px] items-center gap-4 border-b border-divider bg-bg pr-2"
      style={{ paddingLeft: mac ? 78 : 16 }}
    >
      <div className="flex min-w-0 flex-1 items-center gap-2.5">
        <Logo />
        <span className="text-[15px] font-bold tracking-[-0.02em] text-text">uHub</span>
        <span className="rounded-md border border-accent-tint-border px-1.5 py-0.5 font-mono text-[11px] text-accent">
          3D
        </span>
      </div>

      <nav aria-label="Asset type" className="relative flex rounded-xl border border-line bg-surface p-1">
        <div
          aria-hidden="true"
          className="tab-indicator absolute left-1 top-1 h-8 w-24 rounded-lg bg-accent"
          style={{
            transform: `translateX(${tabIndex * 96}px)`,
            boxShadow: "0 6px 20px -6px rgba(212,255,58,.55)",
          }}
        />
        {TABS.map((t) => (
          <button
            key={t.type}
            type="button"
            aria-pressed={tab === t.type}
            onClick={() => setTab(t.type)}
            className={`relative z-10 h-8 w-24 rounded-lg text-[13px] font-medium tracking-[.01em] transition-colors duration-200 ${
              tab === t.type ? "text-accent-ink" : "text-text-3 hover:text-text"
            }`}
          >
            {t.label}
          </button>
        ))}
      </nav>

      <div className="flex min-w-0 flex-1 items-center justify-end gap-2">
        <label className="flex h-9 w-[260px] max-w-full items-center gap-2 rounded-[10px] border border-line bg-surface px-3 text-text-3">
          <SearchIcon />
          <span className="sr-only">Search assets</span>
          <input
            id="asset-search"
            aria-label="Search assets"
            type="search"
            value={search}
            onChange={(e) => setSearch(e.target.value)}
            placeholder="Search name or tag"
            className="min-w-0 flex-1 bg-transparent text-[13px] text-text outline-none placeholder:text-text-3"
          />
          <Kbd>{mac ? "⌘ K" : "Ctrl K"}</Kbd>
        </label>

        <div className="relative" ref={popoverRef}>
          <button
            type="button"
            aria-haspopup="menu"
            aria-expanded={popoverOpen}
            onClick={() => setPopoverOpen((o) => !o)}
            className="inline-flex h-9 shrink-0 items-center gap-2 whitespace-nowrap rounded-[10px] bg-accent px-3.5 text-[13px] font-medium text-accent-ink transition-all duration-150 hover:bg-[#E4FF6A] active:scale-[.97]"
          >
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.4" strokeLinecap="round" aria-hidden="true">
              <path d="M12 5v14M5 12h14" />
            </svg>
            New asset
          </button>
          {popoverOpen && (
            <div
              role="menu"
              aria-label="New asset"
              className="anim-pop absolute right-0 top-full z-50 mt-2 w-60 rounded-xl border border-line-strong bg-surface p-1.5 shadow-2xl"
            >
              {NEW_ITEMS.map((item) => (
                <button
                  key={item.type}
                  type="button"
                  role="menuitem"
                  onClick={() => {
                    setPopoverOpen(false);
                    requestNewAsset(item.type);
                  }}
                  className="flex w-full items-center justify-between rounded-lg px-3 py-2 text-left text-[13px] text-text transition-colors duration-150 hover:bg-raised"
                >
                  {item.label}
                  {item.hint && <Kbd>{mac ? `⌘ ${item.hint}` : `Ctrl ${item.hint}`}</Kbd>}
                </button>
              ))}
            </div>
          )}
        </div>

        <IconButton label="Preferences" className="h-9 w-9 rounded-[10px]" onClick={() => requestPreferences()}>
          <SlidersIcon />
        </IconButton>

        {!mac && (
          <>
            <div className="mx-1 h-5 w-px bg-line" />
            <WindowControls className="h-[52px]" />
          </>
        )}
      </div>
    </header>
  );
}
