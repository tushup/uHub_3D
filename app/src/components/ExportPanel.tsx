import { useEffect, useState } from "react";

import { exportAsset, type ConnectionInfo } from "../api";
import { useStore } from "../store";
import Eyebrow from "./ui/Eyebrow";

const KNOWN_DCCS: { key: string; label: string }[] = [
  { key: "blender", label: "Blender" },
  { key: "maya", label: "Maya" },
  { key: "unreal", label: "Unreal" },
];

const RENDERER_LABELS: Record<string, string> = {
  cycles: "Cycles",
  eevee: "EEVEE",
  arnold: "Arnold",
  unreal: "Unreal",
};

export function dccDisplayName(dcc: string): string {
  return KNOWN_DCCS.find((k) => k.key === dcc.toLowerCase())?.label ?? dcc;
}

export function majorMinor(version: string): string {
  return version.split(".").slice(0, 2).join(".");
}

function errMessage(e: unknown): string {
  return e instanceof Error ? e.message : String(e);
}

export default function ExportPanel({ dir }: { dir: string }) {
  const connections = useStore((s) => s.connections);
  const bridgeStatus = useStore((s) => s.bridgeStatus);
  const lastExport = useStore((s) => s.lastExport);
  const setLastExport = useStore((s) => s.setLastExport);
  const toast = useStore((s) => s.toast);
  const [connId, setConnId] = useState<number | undefined>(undefined);
  const [renderer, setRenderer] = useState<string | undefined>(undefined);
  const [exporting, setExporting] = useState(false);

  const offline = bridgeStatus?.state === "offline";
  const selected: ConnectionInfo | undefined = connections.find((c) => c.id === connId);

  // Preselect the remembered choice while it is still connected.
  useEffect(() => {
    if (lastExport.connId !== undefined) {
      const remembered = connections.find((c) => c.id === lastExport.connId);
      if (remembered) {
        setConnId(remembered.id);
        if (lastExport.renderer && remembered.renderers.includes(lastExport.renderer)) {
          setRenderer(lastExport.renderer);
        }
      }
    }
  }, [connections, lastExport.connId, lastExport.renderer]);

  // A single renderer is preselected; a vanished target clears.
  useEffect(() => {
    if (connId !== undefined && !connections.some((c) => c.id === connId)) {
      setConnId(undefined);
      setRenderer(undefined);
      return;
    }
    if (selected && selected.renderers.length === 1) {
      setRenderer(selected.renderers[0]);
    } else if (selected && renderer !== undefined && !selected.renderers.includes(renderer)) {
      setRenderer(undefined);
    }
  }, [connections, connId, selected, renderer]);

  function chooseTarget(conn: ConnectionInfo) {
    setConnId(conn.id);
    if (conn.renderers.length === 1) {
      setRenderer(conn.renderers[0]);
    } else if (renderer === undefined || !conn.renderers.includes(renderer)) {
      setRenderer(undefined);
    }
  }

  async function onExport() {
    if (connId === undefined || renderer === undefined || exporting || offline) return;
    setExporting(true);
    try {
      const result = await exportAsset(connId, renderer, dir);
      setLastExport({ connId, renderer });
      if (result.ok) {
        toast("success", result.message);
      } else {
        toast("error", result.message);
      }
    } catch (e) {
      toast("error", errMessage(e));
    } finally {
      setExporting(false);
    }
  }

  const canExport =
    connId !== undefined && renderer !== undefined && !exporting && !offline;

  return (
    <div>
      <div className="mb-2.5 flex items-center justify-between">
        <Eyebrow>Export to DCC</Eyebrow>
      </div>

      <div role="radiogroup" aria-label="DCC" className="flex flex-wrap gap-1.5">
        {connections.map((conn) => {
          const isSelected = conn.id === connId;
          return (
            <button
              key={conn.id}
              type="button"
              role="radio"
              aria-checked={isSelected}
              onClick={() => chooseTarget(conn)}
              className={`flex h-[34px] items-center justify-center rounded-[9px] border px-2.5 text-xs transition-colors duration-150 ${
                isSelected
                  ? "border-accent text-text"
                  : "border-line text-text-2 hover:text-text"
              }`}
            >
              {dccDisplayName(conn.dcc)} {majorMinor(conn.dccVersion)} · {conn.pid}
            </button>
          );
        })}
        {KNOWN_DCCS.filter(
          (known) => !connections.some((c) => c.dcc.toLowerCase() === known.key),
        ).map((known) => (
          <span
            key={known.key}
            aria-hidden="true"
            className="flex h-[34px] items-center justify-center rounded-[9px] border border-line px-2.5 text-xs text-text-2 opacity-45"
          >
            {known.label}
          </span>
        ))}
      </div>

      {selected && selected.renderers.length > 0 && (
        <div role="radiogroup" aria-label="Renderer" className="mt-2 flex gap-1.5">
          {selected.renderers.map((r) => {
            const isSelected = r === renderer;
            return (
              <button
                key={r}
                type="button"
                role="radio"
                aria-checked={isSelected}
                onClick={() => setRenderer(r)}
                className={`flex h-[30px] flex-1 items-center justify-center rounded-[9px] border text-xs transition-colors duration-150 ${
                  isSelected
                    ? "border-accent text-text"
                    : "border-line text-text-2 hover:text-text"
                }`}
              >
                {RENDERER_LABELS[r] ?? r}
              </button>
            );
          })}
        </div>
      )}

      {connections.length === 0 && (
        <p className="mt-2.5 text-xs leading-relaxed text-text-3">
          Open Blender, Maya or Unreal with the uHub3D plugin enabled.
        </p>
      )}

      {offline && bridgeStatus?.state === "offline" && (
        <p className="mt-2.5 text-xs font-medium text-danger-text">
          {bridgeStatus.reason.toLowerCase().includes("in use")
            ? `Bridge offline: port ${bridgeStatus.port} in use`
            : `Bridge offline: ${bridgeStatus.reason}`}
        </p>
      )}

      <button
        type="button"
        disabled={!canExport}
        onClick={() => void onExport()}
        className="mt-3 inline-flex h-9 w-full items-center justify-center gap-2 rounded-[10px] bg-accent px-4 text-[13px] font-medium text-accent-ink transition-all duration-150 hover:bg-[#E4FF6A] active:scale-[.97] disabled:pointer-events-none disabled:opacity-40"
      >
        {exporting && (
          <span className="inline-block h-3.5 w-3.5 animate-spin rounded-full border-2 border-accent-ink/30 border-t-accent-ink" />
        )}
        {exporting ? "Exporting…" : "Export"}
      </button>
    </div>
  );
}
