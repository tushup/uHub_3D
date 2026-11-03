// Interactive framing viewport for the New 3D Asset and New
// Material dialogs (spec §A1.1/A1.2). Left-drag rotates the model,
// the wheel zooms, double-click resets. Rendering is on demand:
// one frame per change, no idle loop.

import { useEffect, useRef, useState } from "react";

import {
  imagePreview,
  previewSource,
  type MaterialDraft,
  type PreviewSource,
} from "../api";
import { applyDrag, applyWheel, DEFAULT_VIEW, type View } from "./viewMath";
import { ViewerScene } from "./scene";

export interface ViewerProps {
  source: PreviewSource | null;
  materials: MaterialDraft[];
  view: View;
  onViewChange: (v: View) => void;
  /** Progressive render frame; overlays the live preview when set. */
  renderFrame: string | null;
}

export default function Viewer({
  source,
  materials,
  view,
  onViewChange,
  renderFrame,
}: ViewerProps) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const sceneRef = useRef<ViewerScene | null>(null);
  const viewRef = useRef(view);
  viewRef.current = view;
  const onViewChangeRef = useRef(onViewChange);
  onViewChangeRef.current = onViewChange;
  const dragRef = useRef<{ x: number; y: number } | null>(null);
  const [textureUrls, setTextureUrls] = useState<Map<string, string>>(new Map());

  // Scene lifecycle.
  useEffect(() => {
    if (!canvasRef.current) return;
    const scene = new ViewerScene(canvasRef.current);
    sceneRef.current = scene;
    return () => {
      scene.dispose();
      sceneRef.current = null;
    };
  }, []);

  // Mesh loading.
  const sourceKey = source ? JSON.stringify(source) : "";
  useEffect(() => {
    const scene = sceneRef.current;
    if (!scene) return;
    if (!source) {
      scene.setMesh(null);
      return;
    }
    let live = true;
    previewSource(source)
      .then((mesh) => {
        if (live) scene.setMesh(mesh);
      })
      .catch(() => {
        if (live) scene.setMesh(null);
      });
    return () => {
      live = false;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [sourceKey]);

  // Texture previews for material maps.
  const texturePaths = materials
    .flatMap((m) => m.attributes.map((a) => a.textureSrc))
    .filter((p): p is string => p !== null);
  const textureKey = [...new Set(texturePaths)].sort().join("\n");
  useEffect(() => {
    let live = true;
    for (const path of textureKey.split("\n").filter((p) => p !== "")) {
      if (textureUrls.has(path)) continue;
      imagePreview(path)
        .then((url) => {
          if (!live) return;
          setTextureUrls((prev) => {
            if (prev.has(path)) return prev;
            const next = new Map(prev);
            next.set(path, url);
            return next;
          });
        })
        .catch(() => {});
    }
    return () => {
      live = false;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [textureKey]);

  // Materials + textures.
  useEffect(() => {
    sceneRef.current?.setMaterials(materials, textureUrls);
  }, [materials, textureUrls]);

  // View.
  useEffect(() => {
    sceneRef.current?.setView(view);
  }, [view]);

  // Wheel zoom needs a non-passive listener.
  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const onWheel = (e: WheelEvent) => {
      e.preventDefault();
      onViewChangeRef.current(applyWheel(viewRef.current, e.deltaY));
    };
    canvas.addEventListener("wheel", onWheel, { passive: false });
    return () => canvas.removeEventListener("wheel", onWheel);
  }, []);

  return (
    <div
      data-testid="viewer"
      className="relative h-full w-full touch-none select-none overflow-hidden"
      onPointerDown={(e) => {
        (e.target as HTMLElement).setPointerCapture(e.pointerId);
        dragRef.current = { x: e.clientX, y: e.clientY };
      }}
      onPointerMove={(e) => {
        const last = dragRef.current;
        if (!last) return;
        const dx = e.clientX - last.x;
        const dy = e.clientY - last.y;
        dragRef.current = { x: e.clientX, y: e.clientY };
        if (dx !== 0 || dy !== 0) {
          onViewChangeRef.current(applyDrag(viewRef.current, dx, dy));
        }
      }}
      onPointerUp={() => {
        dragRef.current = null;
      }}
      onPointerCancel={() => {
        dragRef.current = null;
      }}
      onDoubleClick={() => onViewChangeRef.current(DEFAULT_VIEW)}
    >
      <canvas ref={canvasRef} className="block h-full w-full" />
      {renderFrame && (
        <img
          src={renderFrame}
          alt=""
          className="pointer-events-none absolute inset-0 h-full w-full object-cover"
        />
      )}
    </div>
  );
}
