import type { ReactNode } from "react";

interface Props {
  index?: string;
  children: ReactNode;
  className?: string;
}

export default function Eyebrow({ index, children, className = "" }: Props) {
  return (
    <div className={`flex items-center gap-2.5 font-mono text-[11px] uppercase tracking-[.12em] text-text-3 ${className}`}>
      {index && <span className="text-accent">{index} </span>}
      {children}
    </div>
  );
}
