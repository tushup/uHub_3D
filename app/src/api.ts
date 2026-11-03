import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export type AssetType = "model" | "material" | "hdri";

export type AttrKind =
  | "BaseColor"
  | "Metallic"
  | "Roughness"
  | "Specular"
  | "Normal"
  | "Height"
  | "AO"
  | "Opacity"
  | "Emissive";

export type NormalY = "OpenGL" | "DirectX";

/** Untagged on the Rust side: a plain number is a scalar, a tuple is a color. */
export type AttrValue = number | [number, number, number];

export interface Attribute {
  kind: AttrKind;
  value?: AttrValue;
  texture: string | null;
  strength?: number;
  normalY?: NormalY;
}

export interface Material {
  originalName: string;
  name: string;
  attributes: Attribute[];
}

export interface ModelInfo {
  file: string;
  format: string;
  meshes: number;
  vertices: number;
}

export interface HdriInfo {
  file: string;
  width: number;
  height: number;
  rotation: number;
  intensity: number;
}

export interface View {
  yaw: number;
  pitch: number;
  zoom: number;
}

export interface Asset {
  schema: number;
  id: string;
  type: AssetType;
  name: string;
  category: string;
  tags: string[];
  created: string;
  thumbnail: string | null;
  model: ModelInfo | null;
  materials: Material[];
  hdri: HdriInfo | null;
  view?: View;
}

export interface AssetEntry {
  dir: string;
  asset: Asset;
}

export interface ParsedMaterial {
  name: string;
  textures: [AttrKind, string][];
}

export interface ParsedModel {
  format: string;
  meshes: number;
  vertices: number;
  materials: ParsedMaterial[];
}

export interface AttributeDraft {
  kind: AttrKind;
  value: AttrValue | null;
  textureSrc: string | null;
  strength: number | null;
  normalY: NormalY | null;
}

export interface MaterialDraft {
  originalName: string;
  name: string;
  attributes: AttributeDraft[];
}

export interface AssetDraft {
  assetType: AssetType;
  name: string;
  category: string;
  tags: string[];
  modelSrc: string | null;
  hdriSrc: string | null;
  thumbnailSrc: string | null;
  materials: MaterialDraft[];
  rotation: number;
  intensity: number;
  view?: View;
}

export interface ConnectionInfo {
  id: number;
  dcc: string;
  dccVersion: string;
  pid: number;
  renderers: string[];
}

export interface JobResult {
  jobId: string;
  ok: boolean;
  message: string;
}

export interface Settings {
  libraryRoot: string;
  port: number;
}

export interface HdrPreview {
  width: number;
  height: number;
  dataUrl: string;
}

export function listAssets(): Promise<AssetEntry[]> {
  return invoke<AssetEntry[]>("list_assets");
}

export function parseModel(path: string): Promise<ParsedModel> {
  return invoke<ParsedModel>("parse_model", { path });
}

export function hdrPreview(path: string, rotation: number, intensity: number): Promise<HdrPreview> {
  return invoke<HdrPreview>("hdr_preview", { path, rotation, intensity });
}

export type PreviewSource = { kind: "file"; path: string } | { kind: "sphere" };

export interface MeshGroup {
  material: string;
  start: number;
  count: number;
}

export interface PreviewMesh {
  positions: Float32Array;
  normals: Float32Array;
  uvs: Float32Array;
  indices: Uint32Array;
  groups: MeshGroup[];
  hasUvs: boolean;
}

/** Parse the flat preview layout produced by `Mesh::to_preview_bytes`. */
export function parsePreviewBytes(buf: ArrayBuffer): PreviewMesh {
  const view = new DataView(buf);
  let off = 0;
  const u32 = () => {
    const v = view.getUint32(off, true);
    off += 4;
    return v;
  };
  const vertexCount = u32();
  const indexCount = u32();
  const groupCount = u32();
  const hasUvs = u32() === 1;
  const floats = (n: number) => {
    const arr = new Float32Array(buf, off, n);
    off += n * 4;
    return new Float32Array(arr);
  };
  const positions = floats(vertexCount * 3);
  const normals = floats(vertexCount * 3);
  const uvs = floats(vertexCount * 2);
  const indices = new Uint32Array(buf, off, indexCount).slice();
  off += indexCount * 4;
  const decoder = new TextDecoder();
  const groups: MeshGroup[] = [];
  for (let g = 0; g < groupCount; g++) {
    const start = u32();
    const count = u32();
    const nameLen = view.getUint16(off, true);
    off += 2;
    const material = decoder.decode(new Uint8Array(buf, off, nameLen));
    off += nameLen;
    groups.push({ material, start, count });
  }
  return { positions, normals, uvs, indices, groups, hasUvs };
}

export async function previewSource(source: PreviewSource): Promise<PreviewMesh> {
  const buf = await invoke<ArrayBuffer>("preview_source", { source });
  return parsePreviewBytes(buf);
}

export function imagePreview(path: string): Promise<string> {
  return invoke<string>("image_preview", { path });
}

export function saveAsset(draft: AssetDraft): Promise<AssetEntry> {
  return invoke<AssetEntry>("save_asset", { draft });
}

export interface PlannedSave {
  dir: string;
  dirName: string;
  relDir: string;
  files: string[];
}

/** The exact folder and files an add will write (backend-computed). */
export function planSave(draft: AssetDraft): Promise<PlannedSave> {
  return invoke<PlannedSave>("plan_save", { draft });
}

export function cancelRender(): Promise<void> {
  return invoke<void>("cancel_render");
}

/** Absolute path of the bundled `uhub` CLI (shown in Preferences). */
export function cliPath(): Promise<string> {
  return invoke<string>("cli_path");
}

export interface RenderProgress {
  samples: number;
  elapsed: number;
  seconds: number;
  previewDataUrl: string;
  denoised: boolean;
}

export function onRenderProgress(cb: (p: RenderProgress) => void): Promise<UnlistenFn> {
  return listen<RenderProgress>("render://progress", (event) => cb(event.payload));
}

export function deleteAsset(dir: string): Promise<void> {
  return invoke<void>("delete_asset", { dir });
}

export function revealAsset(dir: string): Promise<void> {
  return invoke<void>("reveal_asset", { dir });
}

export function getConnections(): Promise<ConnectionInfo[]> {
  return invoke<ConnectionInfo[]>("get_connections");
}

export type BridgeStatus =
  | { state: "online"; port: number }
  | { state: "offline"; port: number; reason: string };

export function getBridgeStatus(): Promise<BridgeStatus> {
  return invoke<BridgeStatus>("get_bridge_status");
}

export function onConnections(cb: (conns: ConnectionInfo[]) => void): Promise<UnlistenFn> {
  return listen<ConnectionInfo[]>("bridge://connections", (event) => cb(event.payload));
}

export function onBridgeStatus(cb: (status: BridgeStatus) => void): Promise<UnlistenFn> {
  return listen<BridgeStatus>("bridge://status", (event) => cb(event.payload));
}

export function exportAsset(connId: number, renderer: string, dir: string): Promise<JobResult> {
  return invoke<JobResult>("export_asset", { connId, renderer, assetDir: dir });
}

export function getSettings(): Promise<Settings> {
  return invoke<Settings>("get_settings");
}

export function setSettings(settings: Settings): Promise<void> {
  return invoke<void>("set_settings", { settings });
}

export function onNewAsset(cb: (type: AssetType) => void): Promise<UnlistenFn> {
  return listen<AssetType>("menu://new-asset", (event) => cb(event.payload));
}

export function onPreferences(cb: () => void): Promise<UnlistenFn> {
  return listen("menu://preferences", () => cb());
}

export function thumbUrl(entry: AssetEntry): string | null {
  if (!entry.asset.thumbnail) return null;
  // The asset id busts the webview cache so a replaced thumbnail.png
  // under the same path is never shown stale.
  return `${convertFileSrc(`${entry.dir}/${entry.asset.thumbnail}`)}?v=${entry.asset.id}`;
}
