import type { ButtonHTMLAttributes, ReactNode } from "react";

export type ButtonVariant = "primary" | "secondary" | "ghost" | "danger";

const VARIANTS: Record<ButtonVariant, string> = {
  primary: "bg-accent text-accent-ink hover:bg-[#E4FF6A]",
  secondary: "bg-raised text-text border border-line hover:border-line-strong",
  ghost: "text-text-2 hover:text-text hover:bg-raised",
  danger: "bg-transparent text-danger-text border border-danger-border hover:bg-danger-bg",
};

interface Props extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: ButtonVariant;
  children: ReactNode;
}

export default function Button({ variant = "secondary", className = "", children, ...rest }: Props) {
  return (
    <button
      type="button"
      className={`inline-flex h-9 items-center justify-center gap-2 rounded-[10px] px-4 text-[13px] font-medium transition-all duration-150 active:scale-[.97] disabled:pointer-events-none disabled:opacity-40 ${VARIANTS[variant]} ${className}`}
      {...rest}
    >
      {children}
    </button>
  );
}
