import { planSave } from "../../api";
import type {
  AssetDraft,
  AssetEntry,
  AssetType,
  AttrKind,
  AttributeDraft,
  ParsedModel,
  PlannedSave,
  View,
} from "../../api";

export interface DraftState extends AssetDraft {
  thumbnailPreview: string | null;
  parsed: ParsedModel | null;
  hdriSize: [number, number] | null;
  /** Auto tonemapped preview, kept so clearing a custom thumbnail restores it. */
  hdriPreview: string | null;
}

export type DraftAction =
  | { type: "setName"; name: string }
  | { type: "setCategory"; category: string }
  | { type: "setTags"; tags: string[] }
  | { type: "setThumbnail"; path: string | null; preview: string | null }
  | { type: "setModel"; path: string; parsed: ParsedModel }
  | { type: "renameMaterial"; index: number; name: string }
  | { type: "addAttribute"; index: number; kind: AttrKind }
  | { type: "updateAttribute"; index: number; attrIndex: number; patch: Partial<AttributeDraft> }
  | { type: "removeAttribute"; index: number; attrIndex: number }
  | { type: "setHdri"; path: string; size: [number, number]; preview: string }
  | { type: "setRotation"; rotation: number }
  | { type: "setIntensity"; intensity: number }
  | { type: "setView"; view: View };

export function initDraft(type: AssetType): DraftState {
  return {
    assetType: type,
    name: "",
    category: "",
    tags: [],
    modelSrc: null,
    hdriSrc: null,
    thumbnailSrc: null,
    materials: type === "material" ? [{ originalName: "", name: "", attributes: [] }] : [],
    rotation: 0,
    intensity: 1,
    view: { yaw: -30, pitch: 15, zoom: 1 },
    thumbnailPreview: null,
    parsed: null,
    hdriSize: null,
    hdriPreview: null,
  };
}

/** Same rule as the Rust `sanitize_name`; empty result is an empty string. */
export function sanitizeName(raw: string): string {
  return raw
    .trim()
    .replace(/ /g, "_")
    .replace(/[^A-Za-z0-9_]/g, "");
}

export function defaultAttribute(kind: AttrKind, textureSrc: string | null): AttributeDraft {
  const base: AttributeDraft = {
    kind,
    value: null,
    textureSrc,
    strength: null,
    normalY: null,
  };
  switch (kind) {
    case "BaseColor":
      return { ...base, value: [0.8, 0.8, 0.8] };
    case "Metallic":
      return { ...base, value: 0 };
    case "Roughness":
    case "Specular":
      return { ...base, value: 0.5 };
    case "Opacity":
      return { ...base, value: 1 };
    case "Normal":
      return { ...base, strength: 1, normalY: "OpenGL" };
    case "Height":
      return { ...base, strength: 0.1 };
    case "Emissive":
      return { ...base, value: [0, 0, 0], strength: 1 };
    case "AO":
      return base;
  }
}

export function draftReducer(s: DraftState, action: DraftAction): DraftState {
  switch (action.type) {
    case "setName":
      return { ...s, name: action.name };
    case "setCategory":
      return { ...s, category: action.category };
    case "setTags":
      return { ...s, tags: action.tags };
    case "setThumbnail":
      return {
        ...s,
        thumbnailSrc: action.path,
        thumbnailPreview:
          s.assetType === "hdri" && action.path === null ? s.hdriPreview : action.preview,
      };
    case "setModel":
      return {
        ...s,
        modelSrc: action.path,
        parsed: action.parsed,
        materials: action.parsed.materials.map((m) => ({
          originalName: m.name,
          name: m.name,
          attributes: m.textures.map(([kind, textureSrc]) => defaultAttribute(kind, textureSrc)),
        })),
      };
    case "renameMaterial":
      return {
        ...s,
        materials: s.materials.map((m, i) => (i === action.index ? { ...m, name: action.name } : m)),
      };
    case "addAttribute":
      return {
        ...s,
        materials: s.materials.map((m, i) =>
          i === action.index
            ? { ...m, attributes: [...m.attributes, defaultAttribute(action.kind, null)] }
            : m,
        ),
      };
    case "updateAttribute":
      return {
        ...s,
        materials: s.materials.map((m, i) =>
          i === action.index
            ? {
                ...m,
                attributes: m.attributes.map((a, j) =>
                  j === action.attrIndex ? { ...a, ...action.patch } : a,
                ),
              }
            : m,
        ),
      };
    case "removeAttribute":
      return {
        ...s,
        materials: s.materials.map((m, i) =>
          i === action.index
            ? { ...m, attributes: m.attributes.filter((_, j) => j !== action.attrIndex) }
            : m,
        ),
      };
    case "setHdri":
      return {
        ...s,
        hdriSrc: action.path,
        hdriSize: action.size,
        thumbnailPreview: action.preview,
        hdriPreview: action.preview,
      };
    case "setRotation":
      return { ...s, rotation: action.rotation };
    case "setIntensity":
      return { ...s, intensity: action.intensity };
    case "setView":
      return { ...s, view: action.view };
  }
}

/** The name a material will be saved under (material type defaults to the asset name). */
export function effectiveMaterialName(s: DraftState, index: number): string {
  const m = s.materials[index];
  if (!m) return "";
  if (s.assetType === "material" && m.name.trim() === "") return s.name;
  return m.name;
}

export function validate(s: DraftState, existing: AssetEntry[]): Record<string, string> {
  const errors: Record<string, string> = {};
  const name = sanitizeName(s.name);
  const category = sanitizeName(s.category);
  if (name === "") {
    errors.name = "Name is required";
  }
  if (category === "") {
    errors.category = "Category is required";
  }
  if (
    name !== "" &&
    category !== "" &&
    existing.some(
      (e) => e.asset.type === s.assetType && e.asset.category === category && e.asset.name === name,
    )
  ) {
    errors.name = `An asset named ${name} already exists in ${category}`;
  }
  if (s.assetType === "model" && !s.modelSrc) {
    errors.model = "Select a model file";
  }
  if (s.assetType === "hdri" && !s.hdriSrc) {
    errors.hdri = "Select an HDRI file";
  }
  const badMaterial = s.materials.some((_, i) => sanitizeName(effectiveMaterialName(s, i)) === "");
  if (badMaterial) {
    errors.materials = "Material names must not be empty";
  }
  return errors;
}

/** Mirror of the Rust `unique_names`: sanitize, `Material_<i+1>` for empty, `_2`… suffixes. */
export function uniqueMaterialNames(raw: string[]): string[] {
  const used = new Set<string>();
  return raw.map((name, i) => {
    const base = sanitizeName(name) || `Material_${i + 1}`;
    let candidate = base;
    let n = 2;
    while (used.has(candidate)) {
      candidate = `${base}_${n}`;
      n += 1;
    }
    used.add(candidate);
    return candidate;
  });
}

/**
 * The exact folder and file list an add will write, computed by the
 * backend (`plan_save`) so naming has one source of truth.
 */
export async function plannedSave(s: DraftState): Promise<PlannedSave> {
  return planSave(toAssetDraft(s));
}

/** Strip UI-only fields and resolve the material-type name default. */
export function toAssetDraft(s: DraftState): AssetDraft {
  const materials =
    s.assetType === "material"
      ? s.materials.map((m) => {
          const resolved = m.name.trim() === "" ? s.name : m.name;
          return { ...m, originalName: resolved, name: resolved };
        })
      : s.materials;
  return {
    assetType: s.assetType,
    name: s.name,
    category: s.category,
    tags: s.tags,
    modelSrc: s.modelSrc,
    hdriSrc: s.hdriSrc,
    thumbnailSrc: s.thumbnailSrc,
    materials,
    rotation: s.rotation,
    intensity: s.intensity,
    view: s.view,
  };
}
