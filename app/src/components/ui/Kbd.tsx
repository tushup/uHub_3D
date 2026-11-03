import type { ReactNode } from "react";

export default function Kbd({ children }: { children: ReactNode }) {
  return (
    <kbd className="inline-flex items-center rounded-[5px] border border-line-strong bg-surface px-[5px] py-[2px] font-mono text-[10px] leading-none text-text-3">
      {children}
    </kbd>
  );
}
