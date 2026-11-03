import type { CSSProperties } from "react";

interface Props {
  size?: number;
  slow?: boolean;
  className?: string;
}

/** CSS 3D wireframe cube (STUDIO_DARK §4). Front face accent, others text. */
export default function WireCube({ size = 110, slow = false, className = "" }: Props) {
  const half = size / 2;
  const front = "1px solid rgba(212,255,58,.75)";
  const side = "1px solid rgba(237,237,239,.3)";
  const faces: { transform: string; border: string; bg: string }[] = [
    { transform: `rotateY(0deg) translateZ(${half}px)`, border: front, bg: "rgba(212,255,58,.05)" },
    { transform: `rotateY(90deg) translateZ(${half}px)`, border: side, bg: "rgba(237,237,239,.02)" },
    { transform: `rotateY(180deg) translateZ(${half}px)`, border: side, bg: "rgba(237,237,239,.02)" },
    { transform: `rotateY(270deg) translateZ(${half}px)`, border: side, bg: "rgba(237,237,239,.02)" },
    { transform: `rotateX(90deg) translateZ(${half}px)`, border: side, bg: "rgba(237,237,239,.02)" },
    { transform: `rotateX(-90deg) translateZ(${half}px)`, border: side, bg: "rgba(237,237,239,.02)" },
  ];
  const inner: CSSProperties = { width: size, height: size };
  return (
    <div
      aria-hidden="true"
      className={className}
      style={{ width: size, height: size, perspective: size * 6 }}
    >
      <div className={slow ? "uh-cube-slow relative" : "uh-cube relative"} style={inner}>
        {faces.map((f, i) => (
          <div
            key={i}
            data-face=""
            className="absolute inset-0"
            style={{ transform: f.transform, border: f.border, background: f.bg }}
          />
        ))}
      </div>
    </div>
  );
}
