import { thumbUrl, type AssetEntry } from "../api";
import { useStore } from "../store";
import Corners from "./ui/Corners";
import { CubeIcon } from "./newasset/placeholders";
import { pad2, sphereStyle } from "./sphere";

const MODEL_BG = "radial-gradient(120% 70% at 50% 115%, #2A251D 0%, #0F0F11 72%)";
const MATERIAL_BG = "radial-gradient(90% 60% at 50% 100%, #1A1A1D 0%, #0F0F11 70%)";
const HDRI_BG = "linear-gradient(180deg, #2A2E33 0%, #3A3F45 49%, #1E2022 51%, #101112 100%)";

export default function AssetCard({ entry, index }: { entry: AssetEntry; index: number }) {
  const selectedDir = useStore((s) => s.selectedDir);
  const select = useStore((s) => s.select);
  const url = thumbUrl(entry);
  const selected = selectedDir === entry.dir;
  const type = entry.asset.type;

  const bg = type === "model" ? MODEL_BG : type === "material" ? MATERIAL_BG : HDRI_BG;

  return (
    <button
      type="button"
      onClick={() => select(entry.dir)}
      aria-pressed={selected}
      className="anim-rise group relative flex flex-col gap-2.5 text-left transition-transform duration-[450ms] ease-out hover:-translate-y-1"
      style={{ animationDelay: `${Math.min(index * 50, 600)}ms` }}
    >
      <span
        className={`relative block ${type === "hdri" ? "aspect-[2/1]" : "aspect-square"} w-full overflow-hidden rounded-[14px] bg-surface transition-colors duration-200 ${
          selected ? "border border-accent" : "border border-line group-hover:border-[#34343A]"
        }`}
      >
        <span
          className="absolute inset-0 flex items-center justify-center transition-transform duration-700 ease-out group-hover:scale-[1.07]"
          style={url ? undefined : { background: bg }}
        >
          {url ? (
            <img
              src={url}
              alt=""
              className={`h-full w-full ${type === "hdri" ? "object-cover" : "object-contain"}`}
            />
          ) : type === "model" ? (
            <span data-testid="placeholder-cube" style={{ color: "rgba(237,237,239,.55)" }}>
              <CubeIcon size={64} />
            </span>
          ) : type === "material" ? (
            <span style={sphereStyle(entry.asset.materials, "58%")} />
          ) : (
            <span data-testid="placeholder-hdri" className="h-full w-full" style={{ background: HDRI_BG }} />
          )}
        </span>
        <Corners
          inset={8}
          className={`transition-opacity duration-200 ${selected ? "opacity-100" : "opacity-0 group-hover:opacity-60"}`}
        />
      </span>
      <span className="flex items-baseline justify-between gap-2 px-0.5">
        <span className="truncate text-[13px] font-medium text-text">{entry.asset.name}</span>
        <span className="font-mono text-[11px] text-text-3">{pad2(index + 1)}</span>
      </span>
    </button>
  );
}
