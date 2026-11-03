import { open } from "@tauri-apps/plugin-dialog";
import { useEffect, useState } from "react";

import { imagePreview, type AttrKind, type AttributeDraft } from "../../api";

const COLOR_KINDS: AttrKind[] = ["BaseColor", "Emissive"];
const SCALAR_KINDS: AttrKind[] = ["Metallic", "Roughness", "Specular", "Opacity"];
const STRENGTH_KINDS: AttrKind[] = ["Normal", "Height", "Emissive"];

const KIND_LABELS: Partial<Record<AttrKind, string>> = { BaseColor: "Base Color" };

const CHECKER: React.CSSProperties = {
  backgroundColor: "#17171A",
  backgroundImage:
    "linear-gradient(45deg, #26262B 25%, transparent 25%, transparent 75%, #26262B 75%), linear-gradient(45deg, #26262B 25%, transparent 25%, transparent 75%, #26262B 75%)",
  backgroundSize: "10px 10px",
  backgroundPosition: "0 0, 5px 5px",
};

function rgbToHex([r, g, b]: [number, number, number]): string {
  const c = (v: number) =>
    Math.round(Math.min(1, Math.max(0, v)) * 255)
      .toString(16)
      .padStart(2, "0");
  return `#${c(r)}${c(g)}${c(b)}`;
}

function hexToRgb(hex: string): [number, number, number] {
  const n = parseInt(hex.slice(1), 16);
  return [((n >> 16) & 255) / 255, ((n >> 8) & 255) / 255, (n & 255) / 255];
}

function baseName(path: string): string {
  return path.split(/[\\/]/).pop() ?? path;
}

interface Props {
  attr: AttributeDraft;
  onPatch: (patch: Partial<AttributeDraft>) => void;
  onRemove: () => void;
}

export default function AttributeRow({ attr, onPatch, onRemove }: Props) {
  const kind = attr.kind;
  const hasTexture = attr.textureSrc !== null;
  const colorValue: [number, number, number] = Array.isArray(attr.value) ? attr.value : [0.8, 0.8, 0.8];
  const scalarValue = typeof attr.value === "number" ? attr.value : 0.5;
  const [preview, setPreview] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    setPreview(null);
    if (attr.textureSrc) {
      imagePreview(attr.textureSrc)
        .then((url) => {
          if (!cancelled) setPreview(url);
        })
        .catch(() => {});
    }
    return () => {
      cancelled = true;
    };
  }, [attr.textureSrc]);

  async function pickTexture() {
    const picked = await open({
      multiple: false,
      filters: [{ name: "Images", extensions: ["jpg", "jpeg", "png", "tga"] }],
    });
    if (typeof picked === "string") onPatch({ textureSrc: picked });
  }

  return (
    <div
      className="grid items-center gap-3 px-3.5 py-2.5"
      style={{ gridTemplateColumns: "110px minmax(0, 1fr) 210px 28px" }}
    >
      <span className="text-[13px] font-medium text-text">{KIND_LABELS[kind] ?? kind}</span>

      <span
        title={hasTexture ? "Texture overrides value" : undefined}
        className={`flex flex-wrap items-center gap-2.5 ${hasTexture ? "opacity-45" : ""}`}
      >
        {COLOR_KINDS.includes(kind) && (
          <span className="flex items-center gap-2">
            <span
              className="relative block h-[22px] w-[22px] shrink-0 overflow-hidden rounded-md border border-[#3A3A40]"
              style={{ background: rgbToHex(colorValue) }}
            >
              <input
                type="color"
                aria-label={`Value for ${kind}`}
                value={rgbToHex(colorValue)}
                onChange={(e) => onPatch({ value: hexToRgb(e.target.value) })}
                className="absolute inset-0 h-full w-full cursor-pointer opacity-0"
              />
            </span>
            <span className="font-mono text-[11px] text-text-2">{rgbToHex(colorValue).toUpperCase()}</span>
          </span>
        )}
        {SCALAR_KINDS.includes(kind) && (
          <span className="flex min-w-24 flex-1 items-center gap-2.5">
            <input
              type="range"
              min={0}
              max={1}
              step={0.01}
              aria-label={`Value for ${kind}`}
              value={scalarValue}
              onChange={(e) => onPatch({ value: Number(e.target.value) })}
              className="uh-range min-w-0 flex-1"
            />
            <span className="w-[34px] text-right font-mono text-xs text-text">{scalarValue.toFixed(2)}</span>
          </span>
        )}
        {kind === "Normal" && (
          <span className="inline-flex rounded-lg bg-raised p-0.5">
            {(["OpenGL", "DirectX"] as const).map((y) => (
              <button
                key={y}
                type="button"
                aria-pressed={attr.normalY === y}
                onClick={() => onPatch({ normalY: y })}
                className={`rounded-md px-2 py-1 font-mono text-[11px] transition-colors duration-150 ${
                  attr.normalY === y ? "bg-text text-accent-ink" : "text-text-2 hover:text-text"
                }`}
              >
                {y}
              </button>
            ))}
          </span>
        )}
        {STRENGTH_KINDS.includes(kind) && (
          <span className="flex items-center gap-1.5 font-mono text-[11px] text-text-3">
            STR
            <input
              type="number"
              step={0.1}
              min={0}
              aria-label={`Strength for ${kind}`}
              value={attr.strength ?? 1}
              onChange={(e) => onPatch({ strength: Number(e.target.value) })}
              className="w-12 bg-transparent font-mono text-xs text-text outline-none"
            />
          </span>
        )}
        {!COLOR_KINDS.includes(kind) && !SCALAR_KINDS.includes(kind) && !STRENGTH_KINDS.includes(kind) && (
          <span className="font-mono text-[11px] text-text-3">texture only</span>
        )}
      </span>

      {attr.textureSrc ? (
        <span className="flex items-center gap-2 rounded-[9px] border border-accent-tint-border bg-accent-tint p-1">
          <button
            type="button"
            aria-label={`Select texture for ${kind}`}
            title="Replace texture"
            onClick={() => void pickTexture()}
            className="block h-[26px] w-[26px] shrink-0 overflow-hidden rounded-md bg-raised"
          >
            {preview && <img src={preview} alt="" className="h-full w-full object-cover" />}
          </button>
          <span className="min-w-0 flex-1 truncate font-mono text-[11px] text-text">
            {baseName(attr.textureSrc)}
          </span>
          <button
            type="button"
            aria-label={`Clear texture for ${kind}`}
            onClick={() => onPatch({ textureSrc: null })}
            className="shrink-0 px-1 text-text-3 transition-colors hover:text-text"
          >
            ×
          </button>
        </span>
      ) : (
        <button
          type="button"
          aria-label={`Select texture for ${kind}`}
          onClick={() => void pickTexture()}
          className="flex items-center gap-2 rounded-[9px] border border-dashed border-[#2E2E33] p-1 text-left text-text-2 transition-colors duration-150 hover:border-line-strong hover:text-text"
        >
          <span className="block h-[26px] w-[26px] shrink-0 rounded-md" style={CHECKER} />
          <span className="text-xs">Select texture</span>
        </button>
      )}

      <button
        type="button"
        aria-label={`Remove ${kind}`}
        onClick={onRemove}
        className="flex items-center justify-center text-[#6E6E76] transition-colors hover:text-danger-text"
      >
        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" aria-hidden="true">
          <path d="M4 7h16M10 11v6M14 11v6M6 7l1 12a2 2 0 0 0 2 2h6a2 2 0 0 0 2-2l1-12M9 7V4h6v3" />
        </svg>
      </button>
    </div>
  );
}
