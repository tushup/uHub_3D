import { useEffect, useState } from "react";

import { deleteAsset, listAssets, revealAsset, thumbUrl, type Material } from "../api";
import { selectedAsset, useStore } from "../store";
import Button from "./ui/Button";
import Chip, { CHIP_LABELS } from "./ui/Chip";
import ExportPanel from "./ExportPanel";
import WireCube from "./ui/WireCube";
import { sphereStyle } from "./sphere";

const MODEL_BG = "radial-gradient(120% 70% at 50% 115%, #2A251D 0%, #0F0F11 72%)";
const MATERIAL_BG = "radial-gradient(90% 60% at 50% 100%, #1A1A1D 0%, #0F0F11 70%)";
const HDRI_BG = "linear-gradient(180deg, #2A2E33 0%, #3A3F45 49%, #1E2022 51%, #101112 100%)";

function errMessage(e: unknown): string {
  return e instanceof Error ? e.message : String(e);
}

interface Spec {
  k: string;
  v: string;
}

export default function DetailPanel() {
  const entry = useStore((s) => selectedAsset(s));
  const select = useStore((s) => s.select);
  const setAssets = useStore((s) => s.setAssets);
  const toast = useStore((s) => s.toast);
  const [confirming, setConfirming] = useState(false);

  useEffect(() => {
    setConfirming(false);
  }, [entry?.dir]);

  useEffect(() => {
    function onKey(e: globalThis.KeyboardEvent) {
      if (e.key === "Escape") select(null);
    }
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [select]);

  if (!entry) return null;
  const { asset } = entry;
  const url = thumbUrl(entry);

  const specs: Spec[] =
    asset.type === "model" && asset.model
      ? [
          { k: "FILE", v: asset.model.file },
          { k: "FORMAT", v: asset.model.format.toUpperCase() },
          { k: "MESHES", v: String(asset.model.meshes) },
          { k: "VERTICES", v: asset.model.vertices.toLocaleString("en-US") },
        ]
      : asset.type === "material" && asset.materials[0]
        ? [
            { k: "ATTRIBUTES", v: String(asset.materials[0].attributes.length) },
            {
              k: "TEXTURES",
              v: String(asset.materials[0].attributes.filter((a) => a.texture !== null).length),
            },
            { k: "CREATED", v: asset.created.slice(0, 10) },
            { k: "FOLDER", v: `${asset.name}/` },
          ]
        : asset.type === "hdri" && asset.hdri
          ? [
              { k: "FILE", v: asset.hdri.file },
              { k: "RESOLUTION", v: `${asset.hdri.width} × ${asset.hdri.height}` },
              { k: "ROTATION", v: `${asset.hdri.rotation}°` },
              { k: "INTENSITY", v: String(asset.hdri.intensity) },
            ]
          : [];

  const badge =
    asset.type === "hdri"
      ? "EQUIRECT · HDR"
      : asset.type === "material"
        ? "MATERIAL"
        : url
          ? "THUMBNAIL"
          : "NO THUMBNAIL · PLACEHOLDER";

  const previewBg = asset.type === "model" ? MODEL_BG : asset.type === "material" ? MATERIAL_BG : HDRI_BG;
  const materialRows: Material[] =
    asset.type === "model" ? asset.materials : asset.type === "material" ? asset.materials : [];

  async function onDelete() {
    if (!entry) return;
    try {
      await deleteAsset(entry.dir);
      setAssets(await listAssets());
      select(null);
    } catch (e) {
      toast("error", errMessage(e));
    }
  }

  return (
    <section aria-label="Asset details" className="flex h-full min-h-0 min-w-0 flex-col bg-sunken">
      <div key={entry.dir} className="anim-slidein flex min-h-0 min-w-0 flex-1 flex-col gap-[18px] overflow-y-auto p-5">
        <div
          className={`relative ${asset.type === "hdri" ? "aspect-[2/1]" : "aspect-square"} w-full shrink-0 overflow-hidden rounded-2xl border border-line`}
        >
          <div
            className="absolute inset-0 flex items-center justify-center"
            style={url ? undefined : { background: previewBg }}
          >
            {url ? (
              <img
                src={url}
                alt=""
                className={`h-full w-full ${asset.type === "hdri" ? "object-cover" : "object-contain"}`}
              />
            ) : asset.type === "model" ? (
              <WireCube size={120} />
            ) : asset.type === "material" ? (
              <span style={sphereStyle(asset.materials, "56%")} />
            ) : (
              <span className="h-full w-full" style={{ background: HDRI_BG }} />
            )}
          </div>
          <div className="absolute bottom-3 left-3 rounded-md border border-[#2A2A2F] bg-[rgba(10,10,11,.7)] px-2 py-[3px] font-mono text-[11px] text-text">
            {badge}
          </div>
        </div>

        <div>
          <h2 className="mb-1.5 text-[22px] font-semibold tracking-[-0.02em] text-text">{asset.name}</h2>
          <div className="flex flex-wrap items-center gap-2">
            <span className="text-[13px] text-text-2">{asset.category}</span>
            {asset.tags.map((t) => (
              <span
                key={t}
                className="rounded-full border border-[#26262B] px-2 py-0.5 font-mono text-[11px] text-text-2"
              >
                #{t}
              </span>
            ))}
          </div>
        </div>

        {specs.length > 0 && (
          <dl className="grid shrink-0 grid-cols-2 gap-px overflow-hidden rounded-xl border border-[#1A1A1E] bg-[#1A1A1E]">
            {specs.map((s) => (
              <div key={s.k} className="bg-sunken px-3 py-2.5">
                <dt className="mb-1 font-mono text-[10px] tracking-[.12em] text-text-3">{s.k}</dt>
                <dd className="truncate font-mono text-xs text-text">{s.v}</dd>
              </div>
            ))}
          </dl>
        )}

        {materialRows.length > 0 && (
          <div className="flex flex-col gap-2">
            <div className="flex items-center justify-between font-mono text-[10px] tracking-[.12em] text-text-3">
              <span>MATERIALS</span>
              <span>
                <span className="text-accent">■</span> texture&nbsp;&nbsp;<span className="text-accent">□</span> value
              </span>
            </div>
            {materialRows.map((m) => (
              <div
                key={m.name}
                className="flex items-center justify-between gap-2 rounded-[10px] border border-[#1A1A1E] bg-surface px-3 py-2.5"
              >
                <span className="truncate text-[13px] font-medium text-text">{m.name}</span>
                {m.attributes.length > 0 && (
                  <span className="flex flex-wrap justify-end gap-1">
                    {m.attributes.map((a) => (
                      <Chip key={a.kind} label={CHIP_LABELS[a.kind]} filled={a.texture !== null} />
                    ))}
                  </span>
                )}
              </div>
            ))}
          </div>
        )}

      </div>

      <div className="shrink-0 border-t border-divider bg-[#0B0B0C] px-5 pb-[18px] pt-4">
        <div className="mb-4 flex gap-2">
          <Button
            variant="secondary"
            className="flex-1 border-[#26262B]"
            onClick={() => {
              revealAsset(entry.dir).catch((e: unknown) => toast("error", errMessage(e)));
            }}
          >
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinejoin="round" aria-hidden="true">
              <path d="M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z" />
            </svg>
            Open folder
          </Button>
          <Button variant="danger" onClick={() => setConfirming(true)}>
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" aria-hidden="true">
              <path d="M4 7h16M10 11v6M14 11v6M6 7l1 12a2 2 0 0 0 2 2h6a2 2 0 0 0 2-2l1-12M9 7V4h6v3" />
            </svg>
            Delete
          </Button>
        </div>
        <ExportPanel dir={entry.dir} />
      </div>

      {confirming && (
        <div className="fixed inset-0 z-40 flex items-center justify-center bg-[rgba(5,5,6,.72)] backdrop-blur-[6px]">
          <div
            role="alertdialog"
            aria-label="Confirm delete"
            className="anim-pop w-[340px] rounded-2xl border border-line-strong bg-surface p-5 shadow-2xl"
          >
            <p className="text-sm text-text">Move &apos;{asset.name}&apos; to trash?</p>
            <div className="mt-5 flex justify-end gap-2">
              <Button variant="ghost" onClick={() => setConfirming(false)}>
                Cancel
              </Button>
              <button
                type="button"
                onClick={() => {
                  setConfirming(false);
                  void onDelete();
                }}
                className="inline-flex h-9 items-center justify-center rounded-[10px] bg-danger px-4 text-[13px] font-medium text-[#2A0B08] transition-all duration-150 hover:bg-[#FF8574] active:scale-[.97]"
              >
                Move to trash
              </button>
            </div>
          </div>
        </div>
      )}
    </section>
  );
}
