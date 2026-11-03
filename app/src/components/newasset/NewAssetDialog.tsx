import { useEffect, useReducer, useRef, useState } from "react";

import {
  cancelRender,
  hdrPreview,
  listAssets,
  onNewAsset,
  onRenderProgress,
  saveAsset,
  type AssetType,
  type PlannedSave,
  type RenderProgress,
} from "../../api";
import { isMacPlatform } from "../../hooks/useShortcuts";
import { SAVE_DIALOG_EVENT } from "../../hooks/useShortcuts";
import { useStore } from "../../store";
import Button from "../ui/Button";
import Corners from "../ui/Corners";
import Eyebrow from "../ui/Eyebrow";
import IconButton from "../ui/IconButton";
import Viewer from "../../viewer/Viewer";
import { DEFAULT_VIEW } from "../../viewer/viewMath";
import CategoryPicker from "./CategoryPicker";
import HdriFields, { HdriDefaults, HdriPreview } from "./HdriFields";
import MaterialEditor from "./MaterialEditor";
import ModelFields from "./ModelFields";
import TagInput from "./TagInput";
import {
  draftReducer,
  initDraft,
  plannedSave,
  sanitizeName,
  toAssetDraft,
  validate,
} from "./draft";

const TITLES: Record<AssetType, string> = {
  model: "New 3D Asset",
  material: "New Material",
  hdri: "New HDRI",
};

const BADGES: Record<AssetType, string> = { model: "3D", material: "MAT", hdri: "HDRI" };


function errMessage(e: unknown): string {
  return e instanceof Error ? e.message : String(e);
}

export default function NewAssetDialog() {
  const [session, setSession] = useState<{ type: AssetType; nonce: number } | null>(null);
  const newAssetRequest = useStore((s) => s.newAssetRequest);

  useEffect(() => {
    if (newAssetRequest) {
      setSession({ type: newAssetRequest.type, nonce: 1_000_000 + newAssetRequest.nonce });
    }
  }, [newAssetRequest]);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let cancelled = false;
    onNewAsset((type) => setSession({ type, nonce: Date.now() }))
      .then((fn) => {
        if (cancelled) fn();
        else unlisten = fn;
      })
      .catch(() => {});
    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, []);

  if (!session) return null;
  return <DialogSession key={session.nonce} type={session.type} onClose={() => setSession(null)} />;
}

function SavesToCard({ planned }: { planned: PlannedSave | null }) {
  const files = planned?.files ?? [];
  // Show the dir relative to the library root's parent; the full
  // path is the tooltip. Until the first plan returns, the
  // <Category>/<Name> placeholders stand in.
  const rel = planned?.relDir ?? "<Category>/<Name>/";
  const parts = rel.split("/").filter((p) => p !== "");
  const lines = files.filter((f) => f !== "asset.json");
  if (files.includes("asset.json")) {
    if (lines.length > 0) lines[lines.length - 1] = `${lines[lines.length - 1]} · asset.json`;
    else lines.push("asset.json");
  }
  return (
    <div className="rounded-xl border border-[#1C1C20] bg-sunken p-3.5" title={planned?.dir}>
      <Eyebrow className="mb-1.5 text-[10px]">Saves To</Eyebrow>
      <div className="break-all font-mono text-xs leading-relaxed text-text">
        {parts.map((p, i) => (
          <span key={i}>
            {i > 0 && "/"}
            <span className={i >= parts.length - 2 ? "text-accent" : undefined}>{p}</span>
          </span>
        ))}
        /
      </div>
      <div className="mt-1.5 font-mono text-[11px] leading-[1.7] text-text-3">
        {lines.map((f) => (
          <div key={f}>{f}</div>
        ))}
      </div>
    </div>
  );
}

function DialogSession({ type, onClose }: { type: AssetType; onClose: () => void }) {
  const [draft, dispatch] = useReducer(draftReducer, type, initDraft);
  const [saving, setSaving] = useState(false);
  const [saveError, setSaveError] = useState<string | null>(null);
  // Errors surface after the first Save attempt or after a field blurs.
  const [saveAttempted, setSaveAttempted] = useState(false);
  const [blurred, setBlurred] = useState<ReadonlySet<string>>(new Set());
  const [progress, setProgress] = useState<RenderProgress | null>(null);
  const cancelRef = useRef(false);
  const markBlurred = (key: string) =>
    setBlurred((prev) => (prev.has(key) ? prev : new Set(prev).add(key)));
  const shown = (key: string, msg: string | undefined) =>
    saveAttempted || blurred.has(key) ? msg : undefined;
  const assets = useStore((s) => s.assets);
  const setAssets = useStore((s) => s.setAssets);
  const setTab = useStore((s) => s.setTab);
  const select = useStore((s) => s.select);
  const toast = useStore((s) => s.toast);
  const mac = isMacPlatform();

  const errors = validate(draft, assets);
  const isValid = Object.keys(errors).length === 0;
  const savedAs = sanitizeName(draft.name);
  const savedCategory = sanitizeName(draft.category);

  // The Saves-to plan comes from the backend so naming has one
  // source of truth. Keep the last good plan while editing.
  const [planned, setPlanned] = useState<PlannedSave | null>(null);
  useEffect(() => {
    let live = true;
    const t = setTimeout(() => {
      plannedSave(draft)
        .then((p) => {
          if (live) setPlanned(p);
        })
        .catch(() => {
          // Invalid drafts have no plan yet; keep the previous one.
        });
    }, 200);
    return () => {
      live = false;
      clearTimeout(t);
    };
  }, [draft]);

  // The HDRI preview is the same tonemap as the thumbnail, so it
  // follows rotation and intensity (debounced).
  const hdriSrc = draft.hdriSrc;
  const hdriSize = draft.hdriSize;
  useEffect(() => {
    if (type !== "hdri" || !hdriSrc) return;
    const t = setTimeout(() => {
      hdrPreview(hdriSrc, draft.rotation, draft.intensity)
        .then((p) => {
          dispatch({
            type: "setHdri",
            path: hdriSrc,
            size: hdriSize ?? [p.width, p.height],
            preview: p.dataUrl,
          });
        })
        .catch(() => {});
    }, 200);
    return () => clearTimeout(t);
  }, [type, hdriSrc, hdriSize, draft.rotation, draft.intensity]);

  useEffect(() => {
    function onKey(e: globalThis.KeyboardEvent) {
      if (e.key === "Escape") onClose();
    }
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);

  async function onSave() {
    if (saving) return;
    setSaveAttempted(true);
    if (!isValid) return;
    setSaving(true);
    setSaveError(null);
    setProgress(null);
    cancelRef.current = false;
    let unlisten: (() => void) | undefined;
    try {
      unlisten = await onRenderProgress((p) => setProgress(p));
      const entry = await saveAsset(toAssetDraft(draft));
      setAssets(await listAssets());
      setTab(entry.asset.type);
      select(entry.dir);
      const typeLabel = entry.asset.type === "model" ? "3D" : entry.asset.type === "hdri" ? "HDRI" : "Material";
      toast(
        "success",
        `Saved ${entry.asset.name}`,
        `${typeLabel} / ${entry.asset.category} · ${planned?.files.length ?? 0} files`,
      );
      onClose();
    } catch (e) {
      // A cancelled render keeps the dialog and the draft, silently.
      if (!cancelRef.current) setSaveError(errMessage(e));
    } finally {
      unlisten?.();
      setProgress(null);
      setSaving(false);
    }
  }
  const onSaveRef = useRef<() => Promise<void>>(async () => {});
  onSaveRef.current = onSave;
  const saveGateRef = useRef({ saving });
  saveGateRef.current = { saving };

  useEffect(() => {
    function onSaveShortcut() {
      if (!saveGateRef.current.saving) {
        void onSaveRef.current();
      }
    }
    window.addEventListener(SAVE_DIALOG_EVENT, onSaveShortcut);
    return () => window.removeEventListener(SAVE_DIALOG_EVENT, onSaveShortcut);
  }, []);

  function onCancelRender() {
    cancelRef.current = true;
    void cancelRender();
  }

  const view = draft.view ?? DEFAULT_VIEW;
  const viewerSource =
    type === "material"
      ? ({ kind: "sphere" } as const)
      : type === "model" && draft.modelSrc
        ? ({ kind: "file", path: draft.modelSrc } as const)
        : null;

  const textureCount = draft.materials.reduce(
    (n, m) => n + m.attributes.filter((a) => a.textureSrc !== null).length,
    0,
  );
  const summary =
    type === "hdri"
      ? draft.hdriSize
        ? `${draft.hdriSize[0]} × ${draft.hdriSize[1]} HDR · thumbnail generated on save`
        : "HDR · thumbnail generated on save"
      : `${draft.materials.length} material${draft.materials.length === 1 ? "" : "s"} · ${textureCount} texture${textureCount === 1 ? "" : "s"}${isValid ? " · ready to save" : ""}`;

  return (
    <div className="fixed inset-0 z-40 flex items-center justify-center bg-[rgba(5,5,6,.72)] p-6 backdrop-blur-[6px]">
      <section
        role="dialog"
        aria-modal="true"
        aria-label={TITLES[type]}
        className="anim-pop flex max-h-full w-[1080px] max-w-full flex-col overflow-hidden rounded-[20px] border border-line-strong bg-surface shadow-[0_40px_120px_-30px_rgba(0,0,0,.9)]"
      >
        <header className="flex items-center justify-between border-b border-[#1C1C20] px-6 py-[18px]">
          <div className="flex items-center gap-3">
            <span className="rounded-md bg-accent px-[7px] py-[3px] font-mono text-[11px] font-medium text-accent-ink">
              {BADGES[type]}
            </span>
            <h2 className="text-lg font-semibold tracking-[-0.02em] text-text">{TITLES[type]}</h2>
          </div>
          <IconButton label="Close" onClick={onClose}>
            <svg width="14" height="14" viewBox="0 0 12 12" stroke="currentColor" strokeWidth="1.4" aria-hidden="true">
              <path d="m2 2 8 8M10 2l-8 8" />
            </svg>
          </IconButton>
        </header>

        <div className="flex min-h-0 flex-1 flex-wrap overflow-y-auto">
          <div className="flex w-full max-w-[420px] flex-[1_1_340px] flex-col gap-3.5 border-r border-[#1C1C20] p-6">
            {type === "hdri" ? (
              <HdriPreview preview={draft.thumbnailPreview} />
            ) : (
              <>
                <div className="relative aspect-square w-full overflow-hidden rounded-2xl border border-line bg-[#0B0B0C]">
                  <Viewer
                    source={viewerSource}
                    materials={draft.materials}
                    view={view}
                    onViewChange={(v) => dispatch({ type: "setView", view: v })}
                    renderFrame={saving ? (progress?.previewDataUrl ?? null) : null}
                  />
                  <Corners inset={14} className="pointer-events-none opacity-100" />
                  {saving && (
                    <div className="absolute inset-x-0 bottom-0 z-20 flex items-center gap-3 bg-[rgba(8,8,9,.85)] px-3.5 py-2.5">
                      <div className="min-w-0 flex-1">
                        <p className="font-mono text-[10px] leading-tight text-text">
                          PATH TRACING {(progress?.elapsed ?? 0).toFixed(1)} / {(progress?.seconds ?? 10).toFixed(1)} s · {progress?.samples ?? 0} spp
                        </p>
                        <div className="mt-1.5 h-[2px] w-full overflow-hidden rounded bg-[#26262B]">
                          <div
                            className="h-full bg-accent transition-[width] duration-300"
                            style={{
                              width: `${Math.min(100, ((progress?.elapsed ?? 0) / (progress?.seconds ?? 10)) * 100)}%`,
                            }}
                          />
                        </div>
                      </div>
                      <Button variant="secondary" aria-label="Cancel render" onClick={onCancelRender}>
                        Cancel
                      </Button>
                    </div>
                  )}
                </div>
                <p className="text-center text-[11px] text-text-3">
                  Drag to rotate · Scroll to zoom · Double-click to reset
                </p>
                <p className="text-center font-mono text-[10px] text-text-3">
                  YAW {Math.round(view.yaw)}° · PITCH {Math.round(view.pitch)}° · ZOOM {view.zoom.toFixed(2)}×
                </p>
              </>
            )}
            <SavesToCard planned={planned} />
          </div>

          <div className="flex min-w-0 flex-[999_1_480px] flex-col gap-7 px-7 py-6">
            <div onBlurCapture={() => markBlurred("name")}>
              <Eyebrow index="01" className="mb-3">
                Name
              </Eyebrow>
              <label htmlFor="asset-name" className="sr-only">
                Asset Name
              </label>
              <input
                id="asset-name"
                aria-label="Asset Name"
                type="text"
                value={draft.name}
                onChange={(e) => dispatch({ type: "setName", name: e.target.value })}
                className="h-12 w-full rounded-[10px] border border-line-strong bg-sunken px-3 text-lg font-medium text-text outline-none transition-colors focus:border-accent"
              />
              {shown("name", errors.name) ? (
                <p className="mt-2 font-mono text-[11px] text-danger-text">{errors.name}</p>
              ) : (
                savedAs !== "" && (
                  <p className="mt-2 flex items-center gap-1.5 font-mono text-[11px] text-text-2">
                    <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="#D4FF3A" strokeWidth="2.4" strokeLinecap="round" aria-hidden="true">
                      <path d="m5 12 5 5 9-10" />
                    </svg>
                    Saved as <span className="text-text">{savedAs}</span>
                    {savedCategory !== "" && <> · name is free in {savedCategory}</>}
                  </p>
                )
              )}
            </div>

            {type === "model" && (
              <div onBlurCapture={() => markBlurred("model")}>
                <Eyebrow index="02" className="mb-3">
                  Source
                </Eyebrow>
                <ModelFields draft={draft} dispatch={dispatch} error={shown("model", errors.model)} />
              </div>
            )}
            {type === "hdri" && (
              <div onBlurCapture={() => markBlurred("hdri")}>
                <Eyebrow index="02" className="mb-3">
                  Source
                </Eyebrow>
                <HdriFields draft={draft} dispatch={dispatch} error={shown("hdri", errors.hdri)} />
              </div>
            )}

            {type !== "hdri" && (
              <div onBlurCapture={() => markBlurred("materials")}>
                <Eyebrow index={type === "model" ? "03" : "02"} className="mb-3">
                  {type === "model" ? "Materials" : "Material"}
                  {type === "model" && (
                    <span className="ml-auto tracking-normal text-text-3 normal-case">
                      double-click a name to rename
                    </span>
                  )}
                </Eyebrow>
                <MaterialEditor draft={draft} dispatch={dispatch} />
                {shown("materials", errors.materials) && <p className="mt-2 text-xs text-danger-text">{errors.materials}</p>}
              </div>
            )}
            {type === "hdri" && (
              <div>
                <Eyebrow index="03" className="mb-3">
                  Defaults
                </Eyebrow>
                <HdriDefaults draft={draft} dispatch={dispatch} />
              </div>
            )}

            <div onBlurCapture={() => markBlurred("category")}>
              <Eyebrow index={type === "material" ? "03" : "04"} className="mb-3">
                Category &amp; Tags
              </Eyebrow>
              <div className="grid grid-cols-2 gap-3">
                <div>
                  <CategoryPicker
                    type={type}
                    value={draft.category}
                    onChange={(category) => dispatch({ type: "setCategory", category })}
                  />
                  {shown("category", errors.category) && <p className="mt-1 text-xs text-danger-text">{errors.category}</p>}
                </div>
                <TagInput tags={draft.tags} onChange={(tags) => dispatch({ type: "setTags", tags })} />
              </div>
            </div>
          </div>
        </div>

        <footer className="flex flex-wrap items-center justify-between gap-4 border-t border-[#1C1C20] bg-[#0E0E10] px-6 py-4">
          <p className={`min-w-0 font-mono text-[11px] ${saveError ? "text-danger-text" : "text-text-3"}`}>
            {saveError ?? summary}
          </p>
          <div className="flex shrink-0 items-center gap-2">
            <Button variant="ghost" onClick={onClose}>
              Cancel
            </Button>
            <button
              type="button"
              aria-label="Save to Library"
              disabled={saving}
              onClick={() => void onSave()}
              className="inline-flex h-[38px] items-center justify-center gap-2 rounded-[10px] bg-accent px-4 text-[13px] font-medium text-accent-ink shadow-[0_8px_24px_-10px_rgba(212,255,58,.6)] transition-all duration-150 hover:bg-[#E4FF6A] active:scale-[.97] disabled:pointer-events-none disabled:opacity-40"
            >
              {saving ? "Saving…" : "Save to Library"}
              <span className="font-mono text-[10px] opacity-60">{mac ? "⌘ ↵" : "Ctrl ↵"}</span>
            </button>
          </div>
        </footer>
      </section>
    </div>
  );
}
