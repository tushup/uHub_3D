import { create } from "zustand";

import type { AssetEntry, AssetType, BridgeStatus, ConnectionInfo, Settings } from "./api";

export interface Toast {
  id: number;
  kind: "success" | "error";
  message: string;
  sub?: string;
}

export interface AppState {
  tab: AssetType;
  category: string | null;
  search: string;
  selectedDir: string | null;
  assets: AssetEntry[];
  connections: ConnectionInfo[];
  bridgeStatus: BridgeStatus | null;
  lastExport: { connId?: number; renderer?: string };
  toasts: Toast[];
  newAssetRequest: { type: AssetType; nonce: number } | null;
  prefsNonce: number;
  newPopoverNonce: number;
  requestNewAsset: (type: AssetType) => void;
  requestPreferences: () => void;
  requestNewPopover: () => void;
  tile: "large" | "small";
  setTile: (tile: "large" | "small") => void;
  settings: Settings | null;
  setSettings: (settings: Settings | null) => void;
  setTab: (tab: AssetType) => void;
  setCategory: (category: string | null) => void;
  setSearch: (search: string) => void;
  select: (dir: string | null) => void;
  setAssets: (assets: AssetEntry[]) => void;
  setConnections: (connections: ConnectionInfo[]) => void;
  setBridgeStatus: (status: BridgeStatus | null) => void;
  setLastExport: (last: { connId?: number; renderer?: string }) => void;
  toast: (kind: Toast["kind"], message: string, sub?: string) => void;
  loaded: boolean;
  setLoaded: (loaded: boolean) => void;
  dismissToast: (id: number) => void;
}

let nextToastId = 1;

export const useStore = create<AppState>((set, get) => ({
  tab: "material",
  category: null,
  search: "",
  selectedDir: null,
  assets: [],
  connections: [],
  bridgeStatus: null,
  lastExport: {},
  toasts: [],
  setTab: (tab) => set({ tab, category: null, selectedDir: null }),
  setCategory: (category) => set({ category }),
  setSearch: (search) => set({ search }),
  select: (dir) => set({ selectedDir: dir }),
  setAssets: (assets) => set({ assets }),
  setConnections: (connections) => set({ connections }),
  setBridgeStatus: (bridgeStatus) => set({ bridgeStatus }),
  setLastExport: (lastExport) => set({ lastExport }),
  toast: (kind, message, sub) => {
    const id = nextToastId++;
    set({ toasts: [...get().toasts, { id, kind, message, sub }] });
    if (kind === "success") {
      setTimeout(() => get().dismissToast(id), 5000);
    }
  },
  dismissToast: (id) => set({ toasts: get().toasts.filter((t) => t.id !== id) }),
  newAssetRequest: null,
  prefsNonce: 0,
  newPopoverNonce: 0,
  requestNewAsset: (type) =>
    set((s) => ({ newAssetRequest: { type, nonce: (s.newAssetRequest?.nonce ?? 0) + 1 } })),
  requestPreferences: () => set((s) => ({ prefsNonce: s.prefsNonce + 1 })),
  requestNewPopover: () => set((s) => ({ newPopoverNonce: s.newPopoverNonce + 1 })),
  loaded: false,
  setLoaded: (loaded) => set({ loaded }),
  tile: "large",
  setTile: (tile) => set({ tile }),
  settings: null,
  setSettings: (settings) => set({ settings }),
}));

export interface FilterState {
  tab: AssetType;
  category: string | null;
  search: string;
  assets: AssetEntry[];
}

export function visibleAssets(s: FilterState): AssetEntry[] {
  const q = s.search.trim().toLowerCase();
  return s.assets
    .filter((e) => e.asset.type === s.tab)
    .filter((e) => s.category === null || e.asset.category === s.category)
    .filter(
      (e) =>
        q === "" ||
        e.asset.name.toLowerCase().includes(q) ||
        e.asset.tags.some((t) => t.toLowerCase().includes(q)),
    )
    .sort((a, b) => a.asset.name.localeCompare(b.asset.name));
}

export function categoriesFor(
  assets: AssetEntry[],
  tab: AssetType,
): { name: string; count: number }[] {
  const counts = new Map<string, number>();
  for (const e of assets) {
    if (e.asset.type !== tab) continue;
    counts.set(e.asset.category, (counts.get(e.asset.category) ?? 0) + 1);
  }
  return [...counts.entries()]
    .map(([name, count]) => ({ name, count }))
    .sort((a, b) => a.name.localeCompare(b.name));
}

export function selectedAsset(s: {
  assets: AssetEntry[];
  selectedDir: string | null;
}): AssetEntry | null {
  return s.assets.find((e) => e.dir === s.selectedDir) ?? null;
}
