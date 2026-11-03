import type { KeyboardEvent } from "react";

import { useStore, visibleAssets } from "../store";
import AssetCard from "./AssetCard";
import Eyebrow from "./ui/Eyebrow";
import IconButton from "./ui/IconButton";
import { TABS } from "./TopBar";
import { pad2 } from "./sphere";

function LargeTilesIcon() {
  return (
    <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" aria-hidden="true">
      <rect x="3" y="3" width="8" height="8" rx="2" />
      <rect x="13" y="3" width="8" height="8" rx="2" />
      <rect x="3" y="13" width="8" height="8" rx="2" />
      <rect x="13" y="13" width="8" height="8" rx="2" />
    </svg>
  );
}

function SmallTilesIcon() {
  return (
    <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" aria-hidden="true">
      <path d="M3 4h4v4H3zM10 4h4v4h-4zM17 4h4v4h-4zM3 10h4v4H3zM10 10h4v4h-4zM17 10h4v4h-4zM3 16h4v4H3zM10 16h4v4h-4zM17 16h4v4h-4z" />
    </svg>
  );
}

export default function AssetGrid() {
  const assets = useStore((s) => s.assets);
  const tab = useStore((s) => s.tab);
  const category = useStore((s) => s.category);
  const search = useStore((s) => s.search);
  const selectedDir = useStore((s) => s.selectedDir);
  const select = useStore((s) => s.select);
  const tile = useStore((s) => s.tile);
  const setTile = useStore((s) => s.setTile);

  const visible = visibleAssets({ tab, category, search, assets });
  const tabDef = TABS.find((t) => t.type === tab) ?? TABS[0];
  const minCard = tile === "large" ? 176 : 128;

  function onKeyDown(e: KeyboardEvent<HTMLDivElement>) {
    if (e.key === "Escape") {
      select(null);
      return;
    }
    const arrows = ["ArrowLeft", "ArrowRight", "ArrowUp", "ArrowDown"];
    if (!arrows.includes(e.key) || visible.length === 0) return;
    e.preventDefault();
    const cols = Math.max(1, Math.floor(e.currentTarget.clientWidth / (minCard + 20)));
    const current = visible.findIndex((v) => v.dir === selectedDir);
    let next = current < 0 ? 0 : current;
    if (e.key === "ArrowRight") next = Math.min(visible.length - 1, next + 1);
    if (e.key === "ArrowLeft") next = Math.max(0, next - 1);
    if (e.key === "ArrowDown") next = Math.min(visible.length - 1, next + cols);
    if (e.key === "ArrowUp") next = Math.max(0, next - cols);
    select(visible[next].dir);
  }

  return (
    <section className="flex h-full min-h-0 flex-col gap-6 overflow-y-auto px-8 pb-10 pt-7">
      <div className="flex flex-wrap items-end justify-between gap-4">
        <div>
          <Eyebrow className="mb-2.5">
            {`Library / ${tabDef.label} / ${category ? category : "All Categories"}`}
          </Eyebrow>
          <h1 className="flex items-start gap-2.5 text-[44px] font-semibold leading-none tracking-[-0.035em] text-text">
            {tabDef.heading}
            <span className="mt-1 font-mono text-[13px] font-medium tracking-normal text-accent">
              {pad2(visible.length)}
            </span>
          </h1>
        </div>
        <div className="flex items-center gap-1.5">
          <IconButton
            label="Large tiles"
            aria-pressed={tile === "large"}
            onClick={() => setTile("large")}
            className={tile === "large" ? "bg-raised text-text" : ""}
          >
            <LargeTilesIcon />
          </IconButton>
          <IconButton
            label="Small tiles"
            aria-pressed={tile === "small"}
            onClick={() => setTile("small")}
            className={tile === "small" ? "bg-raised text-text" : ""}
          >
            <SmallTilesIcon />
          </IconButton>
        </div>
      </div>

      {visible.length === 0 ? (
        <div className="flex flex-1 items-center justify-center text-center text-sm text-text-2">
          {search.trim() !== "" ? (
            <p>No assets match &apos;{search}&apos;.</p>
          ) : (
            <p>Library is empty. Use File › New Asset to add one.</p>
          )}
        </div>
      ) : (
        <div
          key={`${tab}|${category ?? ""}|${search}`}
          role="listbox"
          aria-label="Assets"
          tabIndex={0}
          onKeyDown={onKeyDown}
          className="grid content-start gap-x-5 gap-y-7"
          style={{ gridTemplateColumns: `repeat(auto-fill, minmax(${minCard}px, 1fr))` }}
        >
          {visible.map((entry, i) => (
            <AssetCard key={entry.dir} entry={entry} index={i} />
          ))}
        </div>
      )}
    </section>
  );
}
