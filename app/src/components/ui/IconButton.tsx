import type { ButtonHTMLAttributes, ReactNode } from "react";

interface Props extends Omit<ButtonHTMLAttributes<HTMLButtonElement>, "aria-label"> {
  /** Required: icon-only buttons must always carry an accessible name. */
  label: string;
  children: ReactNode;
}

export default function IconButton({ label, className = "", children, ...rest }: Props) {
  return (
    <button
      type="button"
      aria-label={label}
      title={label}
      className={`inline-flex h-7 w-7 shrink-0 items-center justify-center rounded-lg text-text-2 transition-all duration-150 hover:bg-raised hover:text-text active:scale-95 disabled:pointer-events-none disabled:opacity-40 ${className}`}
      {...rest}
    >
      {children}
    </button>
  );
}
