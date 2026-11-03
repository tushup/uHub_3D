// three.js scene for the framing viewer (spec §A1.3). Raster
// preview only: the saved thumbnail is the Rust path tracer's
// render of the same view, so this setup mirrors the shared view
// math and stays close in lighting, not pixel-exact.

import * as THREE from "three";

import type { MaterialDraft, PreviewMesh } from "../api";
import { framing, type Bounds, type View } from "./viewMath";

type Vec3 = [number, number, number];

const sub = (a: Vec3, b: Vec3): Vec3 => [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
const scale = (a: Vec3, s: number): Vec3 => [a[0] * s, a[1] * s, a[2] * s];
const dot = (a: Vec3, b: Vec3): number => a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
const cross = (a: Vec3, b: Vec3): Vec3 => [
  a[1] * b[2] - a[2] * b[1],
  a[2] * b[0] - a[0] * b[2],
  a[0] * b[1] - a[1] * b[0],
];
const normalize = (a: Vec3): Vec3 => {
  const len = Math.hypot(a[0], a[1], a[2]) || 1;
  return [a[0] / len, a[1] / len, a[2] / len];
};

/** Rect area lights from the §A1.3 table: position, side, E, colour. */
const LIGHT_TABLE: {
  position: Vec3;
  side: number;
  irradiance: number;
  color: Vec3;
}[] = [
  { position: [-2.0, 2.0, 2.4], side: 1.0, irradiance: 3.0, color: [1.0, 0.96, 0.9] },
  { position: [2.6, 0.6, 2.0], side: 1.6, irradiance: 1.0, color: [0.9, 0.95, 1.0] },
  { position: [1.2, 2.2, -2.6], side: 0.8, irradiance: 4.0, color: [1.0, 1.0, 1.0] },
];

interface BakeLight {
  center: Vec3;
  size: number;
  radiance: Vec3;
  u: Vec3;
  v: Vec3;
  normal: Vec3;
}

/** Same frame construction as the path tracer's RectLight::new. */
function bakeLight(def: (typeof LIGHT_TABLE)[number]): BakeLight {
  const d = Math.hypot(...def.position);
  const normal = normalize(scale(def.position, -1));
  const radiance = scale(def.color, (def.irradiance * d * d) / (def.side * def.side));
  const helper: Vec3 = Math.abs(normal[1]) < 0.9 ? [0, 1, 0] : [1, 0, 0];
  const u = normalize(cross(helper, normal));
  const v = cross(normal, u);
  return { center: def.position, size: def.side, radiance, u, v, normal };
}

const BAKE_LIGHTS = LIGHT_TABLE.map(bakeLight);

/**
 * Bake the studio environment (spec §A1.3 gradient + the three
 * softboxes) into an equirect RGBA float image, linear HDR. A texel
 * whose direction hits a light's square takes that light's radiance
 * (nearest hit), exactly what the path tracer's environment lookup
 * would see behind the geometry.
 */
export function bakeStudioEnv(width = 256, height = 128): Float32Array {
  const data = new Float32Array(width * height * 4);
  for (let y = 0; y < height; y++) {
    const elev = ((y + 0.5) / height - 0.5) * Math.PI;
    const dirY = Math.sin(elev);
    const horiz = Math.cos(elev);
    for (let x = 0; x < width; x++) {
      const az = ((x + 0.5) / width - 0.5) * 2 * Math.PI;
      const dir: Vec3 = [Math.cos(az) * horiz, dirY, Math.sin(az) * horiz];
      const g = dirY < 0 ? 0.06 + dirY * 0.04 : 0.06 + dirY * 0.14;
      let rgb: Vec3 = [g, g, g];
      let bestT = Infinity;
      for (const light of BAKE_LIGHTS) {
        const denom = dot(dir, light.normal);
        if (denom >= 0) continue;
        const t = dot(light.center, light.normal) / denom;
        if (t <= 0 || t >= bestT) continue;
        const p = scale(dir, t);
        const local = sub(p, light.center);
        if (
          Math.abs(dot(local, light.u)) <= light.size / 2 &&
          Math.abs(dot(local, light.v)) <= light.size / 2
        ) {
          bestT = t;
          rgb = light.radiance;
        }
      }
      const i = (y * width + x) * 4;
      data[i] = rgb[0];
      data[i + 1] = rgb[1];
      data[i + 2] = rgb[2];
      data[i + 3] = 1;
    }
  }
  return data;
}

function scalarValue(v: number | [number, number, number] | null | undefined): number | null {
  return typeof v === "number" ? v : null;
}

function draftForGroup(
  groupName: string,
  drafts: MaterialDraft[],
): MaterialDraft | null {
  if (drafts.length === 1) return drafts[0];
  return drafts.find((d) => d.originalName === groupName) ?? null;
}

export class ViewerScene {
  private renderer: THREE.WebGLRenderer;
  private scene = new THREE.Scene();
  private camera: THREE.PerspectiveCamera;
  private modelGroup = new THREE.Group();
  private pmrem: THREE.PMREMGenerator;
  private textures = new Map<string, THREE.Texture>();
  private loader = new THREE.TextureLoader();
  private bounds: Bounds | null = null;
  private view: View = { yaw: -30, pitch: 15, zoom: 1 };
  private disposed = false;

  constructor(canvas: HTMLCanvasElement) {
    this.renderer = new THREE.WebGLRenderer({
      canvas,
      alpha: true,
      antialias: true,
    });
    this.renderer.setSize(512, 512, false);
    this.renderer.toneMapping = THREE.AgXToneMapping;
    this.renderer.toneMappingExposure = 1;
    this.camera = new THREE.PerspectiveCamera(30, 1, 0.01, 100);
    this.camera.position.set(0, 0, 4);

    // Lighting comes only from the baked studio environment (the
    // §A1.3 gradient with the three softboxes in it) through PMREM,
    // so the raster preview matches the path tracer's soft light.
    // No analytic lights; the background stays transparent.
    const envTex = new THREE.DataTexture(
      bakeStudioEnv(),
      256,
      128,
      THREE.RGBAFormat,
      THREE.FloatType,
    );
    envTex.mapping = THREE.EquirectangularReflectionMapping;
    envTex.needsUpdate = true;
    this.pmrem = new THREE.PMREMGenerator(this.renderer);
    const env = this.pmrem.fromEquirectangular(envTex);
    this.scene.environment = env.texture;
    envTex.dispose();

    this.modelGroup.matrixAutoUpdate = false;
    this.scene.add(this.modelGroup);
  }

  setMesh(mesh: PreviewMesh | null) {
    while (this.modelGroup.children.length > 0) {
      const child = this.modelGroup.children.pop() as THREE.Mesh;
      this.modelGroup.remove(child);
      child.geometry.dispose();
      const mats = Array.isArray(child.material) ? child.material : [child.material];
      for (const m of mats) m.dispose();
    }
    this.bounds = null;
    this.meshGroups = [];
    if (!mesh || mesh.positions.length === 0) {
      this.render();
      return;
    }
    const geometry = new THREE.BufferGeometry();
    geometry.setAttribute("position", new THREE.Float32BufferAttribute(mesh.positions, 3));
    geometry.setAttribute("normal", new THREE.Float32BufferAttribute(mesh.normals, 3));
    // The sphere mesh already carries U in 0..2 (mesh.rs); no
    // viewer-side tiling compensation.
    geometry.setAttribute("uv", new THREE.Float32BufferAttribute(mesh.uvs, 2));
    geometry.setIndex(new THREE.BufferAttribute(mesh.indices, 1));

    const min: [number, number, number] = [Infinity, Infinity, Infinity];
    const max: [number, number, number] = [-Infinity, -Infinity, -Infinity];
    for (let i = 0; i < mesh.positions.length; i += 3) {
      for (let a = 0; a < 3; a++) {
        min[a] = Math.min(min[a], mesh.positions[i + a]);
        max[a] = Math.max(max[a], mesh.positions[i + a]);
      }
    }
    this.bounds = { min, max };

    this.meshGroups = mesh.groups.map((g) => ({ name: g.material }));
    const materials: THREE.Material[] = [];
    if (mesh.groups.length > 0) {
      for (const [gi, g] of mesh.groups.entries()) {
        geometry.addGroup(g.start, g.count, gi);
        materials.push(this.buildMaterial(draftForGroup(g.material, this.currentDrafts)));
      }
    }
    const threeMesh = new THREE.Mesh(
      geometry,
      materials.length > 0 ? materials : this.buildMaterial(draftForGroup("", this.currentDrafts)),
    );
    threeMesh.name = "model";
    this.modelGroup.add(threeMesh);
    this.applyFraming();
    this.render();
  }

  private currentDrafts: MaterialDraft[] = [];
  private currentTextureUrls = new Map<string, string>();

  setMaterials(drafts: MaterialDraft[], textureUrls: Map<string, string>) {
    this.currentDrafts = drafts;
    // Load new textures, dispose ones no longer referenced.
    const wanted = new Set<string>();
    for (const d of drafts) {
      for (const a of d.attributes) {
        if (a.textureSrc) wanted.add(a.textureSrc);
      }
    }
    for (const [path, tex] of this.textures) {
      if (!wanted.has(path)) {
        tex.dispose();
        this.textures.delete(path);
      }
    }
    for (const path of wanted) {
      const url = textureUrls.get(path);
      if (!url || this.currentTextureUrls.get(path) === url) continue;
      this.currentTextureUrls.set(path, url);
      const old = this.textures.get(path);
      if (old) old.dispose();
      const tex = this.loader.load(url, () => this.render());
      tex.wrapS = THREE.RepeatWrapping;
      tex.wrapT = THREE.RepeatWrapping;
      this.textures.set(path, tex);
    }
    // Rebuild group materials with the fresh texture set.
    const threeMesh = this.modelGroup.children.find(
      (c): c is THREE.Mesh => c instanceof THREE.Mesh,
    );
    if (threeMesh) {
      const current = Array.isArray(threeMesh.material)
        ? threeMesh.material
        : [threeMesh.material];
      for (const m of current) m.dispose();
      if (this.meshGroups.length > 0) {
        threeMesh.material = this.meshGroups.map((g) =>
          this.buildMaterial(draftForGroup(g.name, drafts)),
        );
      } else {
        threeMesh.material = this.buildMaterial(draftForGroup("", drafts));
      }
    }
    this.render();
  }

  private meshGroups: { name: string }[] = [];

  private buildMaterial(draft: MaterialDraft | null): THREE.MeshPhysicalMaterial {
    const m = new THREE.MeshPhysicalMaterial();
    m.color.setRGB(0.8, 0.8, 0.8);
    m.roughness = 0.5;
    m.metalness = 0;
    if (!draft) return m;
    const texOf = (src: string | null | undefined) =>
      src ? (this.textures.get(src) ?? null) : null;
    for (const a of draft.attributes) {
      const tex = texOf(a.textureSrc);
      switch (a.kind) {
        case "BaseColor":
          if (tex) {
            tex.colorSpace = THREE.SRGBColorSpace;
            m.map = tex;
            m.color.setRGB(1, 1, 1);
          } else if (Array.isArray(a.value)) {
            m.color.setRGB(a.value[0], a.value[1], a.value[2]);
          }
          break;
        case "Roughness": {
          const s = scalarValue(a.value);
          if (tex) m.roughnessMap = tex;
          if (s !== null) m.roughness = s;
          break;
        }
        case "Metallic": {
          const s = scalarValue(a.value);
          if (tex) m.metalnessMap = tex;
          if (s !== null) m.metalness = s;
          break;
        }
        case "Specular": {
          const s = scalarValue(a.value);
          m.specularIntensity = 2 * (s ?? 0.5);
          break;
        }
        case "Opacity": {
          // Opacity is coverage, as in the path tracer (D-31):
          // alphaHash gives stochastic partial coverage instead
          // of a hard alphaTest cut.
          const s = scalarValue(a.value);
          if (tex) m.alphaMap = tex;
          if (s !== null) m.opacity = s;
          if (tex || (s !== null && s < 1)) m.alphaHash = true;
          break;
        }
        case "Emissive":
          if (tex) {
            tex.colorSpace = THREE.SRGBColorSpace;
            m.emissiveMap = tex;
            if (Array.isArray(a.value)) {
              m.emissive.setRGB(a.value[0], a.value[1], a.value[2]);
            } else {
              m.emissive.setRGB(1, 1, 1);
            }
          } else if (Array.isArray(a.value)) {
            m.emissive.setRGB(a.value[0], a.value[1], a.value[2]);
          }
          break;
        case "Normal":
          if (tex) {
            m.normalMap = tex;
            const strength = a.strength ?? 1;
            m.normalScale.set(strength, strength * (a.normalY === "DirectX" ? -1 : 1));
          }
          break;
        case "AO":
          if (tex) m.aoMap = tex;
          break;
        case "Height":
          break;
      }
    }
    return m;
  }

  setView(view: View) {
    this.view = view;
    this.applyFraming();
    this.render();
  }

  private applyFraming() {
    if (!this.bounds) {
      this.camera.position.set(0, 0, 4);
      this.modelGroup.matrix.identity();
      return;
    }
    const f = framing(this.bounds, this.view);
    const m = new THREE.Matrix4();
    const rows = f.model;
    m.set(
      rows[0][0], rows[0][1], rows[0][2], rows[0][3],
      rows[1][0], rows[1][1], rows[1][2], rows[1][3],
      rows[2][0], rows[2][1], rows[2][2], rows[2][3],
      rows[3][0], rows[3][1], rows[3][2], rows[3][3],
    );
    this.modelGroup.matrix.copy(m);
    this.camera.position.set(0, 0, f.distance);
  }

  render() {
    if (this.disposed) return;
    this.renderer.render(this.scene, this.camera);
  }

  dispose() {
    this.disposed = true;
    for (const tex of this.textures.values()) tex.dispose();
    this.textures.clear();
    this.pmrem.dispose();
    this.renderer.dispose();
  }
}
