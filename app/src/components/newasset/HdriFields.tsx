import { open } from "@tauri-apps/plugin-dialog";
import { useRef, useState, type Dispatch, type KeyboardEvent, type PointerEvent } from "react";

import { hdrPreview } from "../../api";
import type { DraftAction, DraftState } from "./draft";
import { SourceCard } from "./ModelFields";

interface Props {
  draft: DraftState;
  dispatch: Dispatch<DraftAction>;
  error?: string;
}

function baseName(path: string): string {
  return path.split(/[\\/]/).pop() ?? path;
}

function normalizeDeg(v: number): number {
  return ((v % 360) + 360) % 360;
}

export async function pickHdri(
  dispatch: Dispatch<DraftAction>,
  setBusy: (b: boolean) => void,
  setError: (e: string | null) => void,
): Promise<void> {
  const picked = await open({
    multiple: false,
    filters: [{ name: "HDRI", extensions: ["hdr"] }],
  });
  if (typeof picked !== "string") return;
  setBusy(true);
  try {
    const preview = await hdrPreview(picked, 0, 1);
    dispatch({ type: "setHdri", path: picked, size: [preview.width, preview.height], preview: preview.dataUrl });
    setError(null);
  } catch (e) {
    setError(e instanceof Error ? e.message : String(e));
  } finally {
    setBusy(false);
  }
}

/**
 * Square HDRI preview slot, the same size and corners as the
 * 3D/Material viewer. The whole tonemapped panorama is fitted to
 * the slot's width (object-contain, centred) over a blurred,
 * darkened cover copy of itself, so the square has no empty bands.
 * There are no probe spheres.
 */
export function HdriPreview({ preview }: { preview: string | null }) {
  return (
    <div
      data-testid="hdri-preview"
      className="relative flex aspect-square w-full flex-col overflow-hidden rounded-2xl border border-line bg-[#0B0B0C]"
    >
      {preview ? (
        <>
          <img
            src={preview}
            alt=""
            aria-hidden="true"
            className="absolute inset-0 h-full w-full object-cover blur-[24px] brightness-[0.45]"
          />
          <img
            src={preview}
            alt=""
            className="absolute inset-0 h-full w-full object-contain"
          />
        </>
      ) : (
        <div
          className="absolute inset-0"
          style={{
            background:
              "linear-gradient(180deg, #2A2E33 0%, #3A3F45 49%, #1E2022 51%, #101112 100%)",
          }}
        />
      )}
      <span aria-hidden="true" className="scan-line absolute top-0 h-full w-px bg-accent shadow-[0_0_12px_#D4FF3A]" />
      <span className="absolute left-3 top-3 rounded-md border border-[#2A2A2F] bg-[rgba(10,10,11,.7)] px-[7px] py-[3px] font-mono text-[10px] text-text">
        AUTO PREVIEW · TONEMAPPED
      </span>
    </div>
  );
}

export default function HdriFields({ draft, dispatch, error }: Props) {
  const [loadError, setLoadError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const shownError = loadError ?? error;

  return (
    <div>
      {draft.hdriSrc ? (
        <SourceCard
          badge="HDR"
          fileName={baseName(draft.hdriSrc)}
          sub={draft.hdriSize ? `${draft.hdriSize[0]} × ${draft.hdriSize[1]} · equirectangular` : draft.hdriSrc}
          onReplace={() => void pickHdri(dispatch, setBusy, setLoadError)}
          busy={busy}
        />
      ) : (
        <button
          type="button"
          onClick={() => void pickHdri(dispatch, setBusy, setLoadError)}
          disabled={busy}
          className="inline-flex h-[38px] items-center rounded-[10px] border border-line-strong bg-raised px-4 text-[13px] font-medium text-text transition-all duration-150 active:scale-[.97] disabled:opacity-40"
        >
          Select HDRI…
        </button>
      )}
      {shownError && <p className="mt-2 text-xs text-danger-text">{shownError}</p>}
    </div>
  );
}

export function HdriDefaults({ draft, dispatch }: Props) {
  const dialRef = useRef<HTMLDivElement>(null);
  const rotation = normalizeDeg(draft.rotation);
  // Round before wrapping so 359.6 shows 0, never 360.
  const shownRotation = Math.round(rotation) % 360;

  function angleFromEvent(e: PointerEvent): number {
    const el = dialRef.current;
    if (!el) return rotation;
    const rect = el.getBoundingClientRect();
    const dx = e.clientX - (rect.left + rect.width / 2);
    const dy = e.clientY - (rect.top + rect.height / 2);
    return normalizeDeg((Math.atan2(dy, dx) * 180) / Math.PI + 90);
  }

  function onDialKey(e: KeyboardEvent) {
    const step = e.shiftKey ? 15 : 1;
    if (e.key === "ArrowRight" || e.key === "ArrowUp") {
      e.preventDefault();
      dispatch({ type: "setRotation", rotation: normalizeDeg(rotation + step) });
    } else if (e.key === "ArrowLeft" || e.key === "ArrowDown") {
      e.preventDefault();
      dispatch({ type: "setRotation", rotation: normalizeDeg(rotation - step) });
    }
  }

  return (
    <div className="grid grid-cols-2 gap-3">
      <div className="flex items-center gap-4 rounded-xl border border-line-strong bg-sunken p-4">
        <div
          ref={dialRef}
          role="slider"
          tabIndex={0}
          aria-label="Rotation"
          aria-valuemin={0}
          aria-valuemax={360}
          aria-valuenow={shownRotation}
          onKeyDown={onDialKey}
          onPointerDown={(e) => {
            (e.target as HTMLElement).setPointerCapture(e.pointerId);
            dispatch({ type: "setRotation", rotation: angleFromEvent(e) });
          }}
          onPointerMove={(e) => {
            if (e.buttons === 1) dispatch({ type: "setRotation", rotation: angleFromEvent(e) });
          }}
          className="relative h-[72px] w-[72px] shrink-0 cursor-pointer touch-none rounded-full border border-[#2E2E33]"
          style={{ background: "radial-gradient(circle, #141416 55%, #0D0D0F 56%)" }}
        >
          <div
            aria-hidden="true"
            className="absolute left-1/2 top-1/2 h-[30px] w-0.5 bg-accent"
            style={{ marginLeft: -1, transformOrigin: "50% 0", transform: `rotate(${rotation + 180}deg)` }}
          />
          <div aria-hidden="true" className="absolute left-1/2 top-1/2 h-2 w-2 rounded-full bg-text" style={{ margin: "-4px 0 0 -4px" }} />
        </div>
        <div>
          <div className="mb-1 text-xs text-text-2">Rotation</div>
          <div className="font-mono text-[28px] font-medium leading-none tracking-[-0.02em] text-text">
            {shownRotation}
            <span className="text-text-3">°</span>
          </div>
        </div>
      </div>

      <div className="flex flex-col justify-center gap-3 rounded-xl border border-line-strong bg-sunken p-4">
        <div className="flex items-baseline justify-between">
          <span className="text-xs text-text-2">Intensity</span>
          <span className="font-mono text-[28px] font-medium leading-none tracking-[-0.02em] text-text">
            {draft.intensity.toFixed(1)}
          </span>
        </div>
        <input
          type="range"
          min={0}
          max={10}
          step={0.1}
          aria-label="Default Intensity"
          value={draft.intensity}
          onChange={(e) => dispatch({ type: "setIntensity", intensity: Number(e.target.value) })}
          className="uh-range w-full"
        />
        <div className="flex justify-between font-mono text-[10px] text-text-3">
          <span>0</span>
          <span>10</span>
        </div>
      </div>
    </div>
  );
}
