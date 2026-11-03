import type { AttrKind } from "../../api";

/** Short mono labels for attribute chips (STUDIO_DARK §4). */
export const CHIP_LABELS: Record<AttrKind, string> = {
  BaseColor: "BC",
  Metallic: "M",
  Roughness: "R",
  Specular: "S",
  Normal: "N",
  Height: "H",
  AO: "AO",
  Opacity: "O",
  Emissive: "E",
};

interface Props {
  label: string;
  filled?: boolean;
}

export default function Chip({ label, filled = false }: Props) {
  if (filled) {
    return (
      <span className="inline-flex items-center rounded-[5px] bg-accent px-[5px] py-[2px] font-mono text-[10px] font-medium leading-none text-accent-ink">
        {label}
      </span>
    );
  }
  return (
    <span className="inline-flex items-center rounded-[5px] border border-accent-chip px-[5px] py-[2px] font-mono text-[10px] leading-none text-accent">
      {label}
    </span>
  );
}
