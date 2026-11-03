import { useEffect } from "react";

import {
  getBridgeStatus,
  getConnections,
  getSettings,
  listAssets,
  onBridgeStatus,
  onConnections,
  type AssetType,
} from "./api";
import { SAVE_DIALOG_EVENT, useShortcuts } from "./hooks/useShortcuts";
import AssetGrid from "./components/AssetGrid";
import DetailPanel from "./components/DetailPanel";
import EmptyState from "./components/EmptyState";
import NewAssetDialog from "./components/newasset/NewAssetDialog";
import PreferencesDialog from "./components/PreferencesDialog";
import CategorySidebar from "./components/CategorySidebar";
import StatusBar from "./components/StatusBar";
import Toasts from "./components/Toasts";
import TopBar from "./components/TopBar";
import { selectedAsset, useStore } from "./store";

const TAB_ORDER: AssetType[] = ["material", "model", "hdri"];

export default function App() {
  const setAssets = useStore((s) => s.setAssets);
  const toast = useStore((s) => s.toast);
  const hasSelection = useStore((s) => selectedAsset(s) !== null);
  const requestNewAsset = useStore((s) => s.requestNewAsset);
  const requestNewPopover = useStore((s) => s.requestNewPopover);
  const requestPreferences = useStore((s) => s.requestPreferences);
  const setTab = useStore((s) => s.setTab);
  const select = useStore((s) => s.select);
  const loaded = useStore((s) => s.loaded);
  const setLoaded = useStore((s) => s.setLoaded);
  const assetCount = useStore((s) => s.assets.length);
  const isEmpty = loaded && assetCount === 0;

  useShortcuts({
    onNewAsset: (type) => requestNewAsset(type),
    onOpenNewPopover: () => requestNewPopover(),
    onFocusSearch: () => document.getElementById("asset-search")?.focus(),
    onPreferences: () => requestPreferences(),
    onTab: (index) => {
      const tab = TAB_ORDER[index];
      if (tab) setTab(tab);
    },
    onSaveDialog: () => window.dispatchEvent(new Event(SAVE_DIALOG_EVENT)),
    onEscape: () => {
      if (!document.querySelector('[role="dialog"]')) select(null);
    },
  });

  const setSettings = useStore((s) => s.setSettings);
  const setConnections = useStore((s) => s.setConnections);
  const setBridgeStatus = useStore((s) => s.setBridgeStatus);

  useEffect(() => {
    listAssets()
      .then(setAssets)
      .catch((e: unknown) => toast("error", e instanceof Error ? e.message : String(e)))
      .finally(() => setLoaded(true));
    getSettings()
      .then(setSettings)
      .catch(() => {});
    getConnections()
      .then(setConnections)
      .catch(() => {});
    getBridgeStatus()
      .then(setBridgeStatus)
      .catch(() => {});
    let unlistenConnections: (() => void) | undefined;
    let unlistenStatus: (() => void) | undefined;
    onConnections(setConnections)
      .then((fn) => {
        unlistenConnections = fn;
      })
      .catch(() => {});
    onBridgeStatus(setBridgeStatus)
      .then((fn) => {
        unlistenStatus = fn;
      })
      .catch(() => {});
    return () => {
      unlistenConnections?.();
      unlistenStatus?.();
    };
  }, [setAssets, toast, setSettings, setLoaded, setConnections, setBridgeStatus]);

  // Re-scan on window focus (D-27): CLI changes appear without a
  // file watcher. Manual deletes from disk resolve this way too.
  useEffect(() => {
    function onFocus() {
      listAssets()
        .then(setAssets)
        .catch(() => {});
    }
    window.addEventListener("focus", onFocus);
    return () => window.removeEventListener("focus", onFocus);
  }, [setAssets]);

  return (
    <div
      className="grid h-screen overflow-hidden bg-bg text-text"
      style={{
        gridTemplateRows: "52px 1fr 30px",
        gridTemplateColumns: hasSelection ? "248px 1fr 380px" : "248px 1fr",
      }}
    >
      <div className="col-span-full min-h-0 min-w-0">
        <TopBar />
      </div>
      {isEmpty ? (
        <main className="col-span-full min-h-0 min-w-0">
          <EmptyState />
        </main>
      ) : (
        <>
          <div className="min-h-0 min-w-0 border-r border-divider">
            <CategorySidebar />
          </div>
          <main className="min-h-0 min-w-0">
            <AssetGrid />
          </main>
          {hasSelection && (
            <div className="min-h-0 min-w-0 border-l border-divider">
              <DetailPanel />
            </div>
          )}
        </>
      )}
      <div className="col-span-full min-h-0 min-w-0">
        <StatusBar />
      </div>
      <Toasts />
      <NewAssetDialog />
      <PreferencesDialog />
    </div>
  );
}
