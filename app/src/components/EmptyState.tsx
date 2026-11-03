import type { AssetType } from "../api";
import { isMacPlatform } from "../hooks/useShortcuts";
import { useStore } from "../store";
import WireCube from "./ui/WireCube";

const BUTTONS: { type: AssetType; label: string; primary?: boolean; hint?: boolean; icon: JSX.Element }[] = [
  {
    type: "model",
    label: "3D Asset",
    primary: true,
    hint: true,
    icon: (
      <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinejoin="round" aria-hidden="true">
        <path d="M12 2.8 20 7.4v9.2L12 21.2 4 16.6V7.4z" />
        <path d="M4 7.4 12 12l8-4.6M12 12v9.2" />
      </svg>
    ),
  },
  {
    type: "material",
    label: "Material",
    icon: (
      <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" aria-hidden="true">
        <circle cx="12" cy="12" r="8.5" />
        <path d="M3.5 12c3 2 14 2 17 0" />
      </svg>
    ),
  },
  {
    type: "hdri",
    label: "HDRI",
    icon: (
      <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" aria-hidden="true">
        <path d="M3 16h18M6 12a6 6 0 0 1 12 0" />
        <path d="M12 3v2M4.5 6.5l1.4 1.4M19.5 6.5l-1.4 1.4" />
      </svg>
    ),
  },
];

export default function EmptyState() {
  const requestNewAsset = useStore((s) => s.requestNewAsset);
  const mac = isMacPlatform();

  return (
    <div className="dot-grid flex h-full items-center justify-center px-6 py-10">
      <div className="flex max-w-[620px] flex-col items-center gap-7 text-center">
        <div className="anim-float pb-10">
          <WireCube size={160} slow />
        </div>
        <div>
          <div className="mb-3.5 font-mono text-[11px] tracking-[.14em] text-accent">LIBRARY · 00 ASSETS</div>
          <h1 className="text-[56px] font-semibold leading-[1.02] tracking-[-0.04em] text-text">
            Your library starts here.
          </h1>
          <p className="mt-4 text-base leading-relaxed text-text-2">
            Add a model, a material or an HDRI. uHub renames and files everything for you, ready to send to
            Blender, Maya or Unreal.
          </p>
        </div>
        <div className="flex flex-wrap justify-center gap-2.5">
          {BUTTONS.map((b) => (
            <button
              key={b.type}
              type="button"
              onClick={() => requestNewAsset(b.type)}
              className={
                b.primary
                  ? "inline-flex h-11 items-center justify-center gap-2.5 rounded-xl bg-accent px-[18px] text-sm font-medium text-accent-ink shadow-[0_10px_30px_-12px_rgba(212,255,58,.6)] transition-all duration-150 hover:bg-[#E4FF6A] active:scale-[.97]"
                  : "inline-flex h-11 items-center justify-center gap-2.5 rounded-xl border border-line-strong bg-surface px-[18px] text-sm font-medium text-text transition-all duration-150 hover:border-[#3A3A40] active:scale-[.97]"
              }
            >
              {b.icon}
              {b.label}
              {b.hint && <span className="font-mono text-[10px] opacity-60">{mac ? "⌘ N" : "Ctrl N"}</span>}
            </button>
          ))}
        </div>
      </div>
    </div>
  );
}
