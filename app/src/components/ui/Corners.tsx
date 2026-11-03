import type { CSSProperties } from "react";

interface Props {
  inset?: number;
  className?: string;
}

const BASE: CSSProperties = {
  width: 12,
  height: 12,
  borderColor: "#D4FF3A",
  borderStyle: "solid",
  borderWidth: 0,
};

/** Viewfinder corner marks. The parent must be position: relative. */
export default function Corners({ inset = 8, className = "" }: Props) {
  const corners: CSSProperties[] = [
    { top: inset, left: inset, borderTopWidth: 1.5, borderLeftWidth: 1.5 },
    { top: inset, right: inset, borderTopWidth: 1.5, borderRightWidth: 1.5 },
    { bottom: inset, left: inset, borderBottomWidth: 1.5, borderLeftWidth: 1.5 },
    { bottom: inset, right: inset, borderBottomWidth: 1.5, borderRightWidth: 1.5 },
  ];
  return (
    <>
      {corners.map((pos, i) => (
        <span
          key={i}
          aria-hidden="true"
          data-corner=""
          className={`pointer-events-none absolute z-10 ${className}`}
          style={{ ...BASE, ...pos }}
        />
      ))}
    </>
  );
}
