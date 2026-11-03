import { open } from "@tauri-apps/plugin-dialog";
import { useState, type Dispatch } from "react";

import { parseModel } from "../../api";
import type { DraftAction, DraftState } from "./draft";

interface Props {
  draft: DraftState;
  dispatch: Dispatch<DraftAction>;
  error?: string;
}

function baseName(path: string): string {
  return path.split(/[\\/]/).pop() ?? path;
}

function extBadge(path: string): string {
  const m = /\.([^.\\/]+)$/.exec(path);
  return m ? m[1].toUpperCase() : "3D";
}

export function SourceCard({
  badge,
  fileName,
  sub,
  onReplace,
  busy,
}: {
  badge: string;
  fileName: string;
  sub: string;
  onReplace: () => void;
  busy?: boolean;
}) {
  return (
    <div className="flex items-center gap-3 rounded-xl border border-line-strong bg-sunken p-3">
      <span className="flex h-11 w-11 shrink-0 items-center justify-center rounded-[10px] bg-raised font-mono text-[11px] text-accent">
        {badge}
      </span>
      <span className="min-w-0 flex-1">
        <span className="block truncate text-sm font-medium text-text">{fileName}</span>
        <span className="block truncate font-mono text-[11px] text-text-3">{sub}</span>
      </span>
      <button
        type="button"
        onClick={onReplace}
        disabled={busy}
        className="inline-flex h-[34px] shrink-0 items-center rounded-[10px] border border-[#26262B] bg-raised px-3.5 text-[13px] font-medium text-text transition-all duration-150 hover:border-line-strong active:scale-[.97] disabled:opacity-40"
      >
        Replace
      </button>
    </div>
  );
}

export default function ModelFields({ draft, dispatch, error }: Props) {
  const [parseError, setParseError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  async function pick() {
    const picked = await open({
      multiple: false,
      filters: [{ name: "3D Model", extensions: ["fbx", "obj"] }],
    });
    if (typeof picked !== "string") return;
    setBusy(true);
    try {
      const parsed = await parseModel(picked);
      dispatch({ type: "setModel", path: picked, parsed });
      setParseError(null);
    } catch (e) {
      setParseError(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(false);
    }
  }

  const shownError = parseError ?? error;

  return (
    <div>
      {draft.modelSrc ? (
        <>
          <SourceCard
            badge={extBadge(draft.modelSrc)}
            fileName={baseName(draft.modelSrc)}
            sub={draft.modelSrc}
            onReplace={() => void pick()}
            busy={busy}
          />
          {draft.parsed && (
            <div className="mt-2.5 flex gap-[18px] font-mono text-[11px] text-text-2">
              <span>
                <span className="text-text">{draft.parsed.meshes}</span> MESHES
              </span>
              <span>
                <span className="text-text">{draft.parsed.vertices.toLocaleString("en-US")}</span> VERTICES
              </span>
              <span>
                <span className="text-text">{draft.parsed.materials.length}</span> MATERIALS
              </span>
            </div>
          )}
        </>
      ) : (
        <button
          type="button"
          onClick={() => void pick()}
          disabled={busy}
          className="inline-flex h-[38px] items-center rounded-[10px] border border-line-strong bg-raised px-4 text-[13px] font-medium text-text transition-all duration-150 hover:border-line-strong active:scale-[.97] disabled:opacity-40"
        >
          Select Asset…
        </button>
      )}
      {shownError && <p className="mt-2 text-xs text-danger-text">{shownError}</p>}
    </div>
  );
}
