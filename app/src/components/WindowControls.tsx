import { getCurrentWindow } from "@tauri-apps/api/window";

function MinIcon() {
  return (
    <svg width="10" height="10" viewBox="0 0 10 10" aria-hidden="true">
      <path d="M1 5h8" stroke="currentColor" strokeWidth="1" />
    </svg>
  );
}

function MaxIcon() {
  return (
    <svg width="10" height="10" viewBox="0 0 10 10" aria-hidden="true">
      <rect x="1" y="1" width="8" height="8" fill="none" stroke="currentColor" strokeWidth="1" />
    </svg>
  );
}

function CloseIcon() {
  return (
    <svg width="10" height="10" viewBox="0 0 10 10" aria-hidden="true">
      <path d="M1 1l8 8M9 1l-8 8" stroke="currentColor" strokeWidth="1" />
    </svg>
  );
}

const base =
  "flex w-[46px] items-center justify-center text-text-2 transition-colors duration-150 hover:bg-raised hover:text-text active:scale-95";

/** Windows/Linux window controls for the frameless title bar. */
export default function WindowControls({ className = "h-12" }: { className?: string }) {
  const win = () => getCurrentWindow();
  return (
    <div className={`flex items-stretch ${className}`}>
      <button type="button" aria-label="Minimize" className={base} onClick={() => void win().minimize()}>
        <MinIcon />
      </button>
      <button
        type="button"
        aria-label="Maximize"
        className={base}
        onClick={() => void win().toggleMaximize()}
      >
        <MaxIcon />
      </button>
      <button
        type="button"
        aria-label="Close"
        className={`${base} hover:bg-[#E81123] hover:text-white`}
        onClick={() => void win().close()}
      >
        <CloseIcon />
      </button>
    </div>
  );
}
