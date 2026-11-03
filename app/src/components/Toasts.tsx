import { useStore } from "../store";

export default function Toasts() {
  const toasts = useStore((s) => s.toasts);
  const dismissToast = useStore((s) => s.dismissToast);

  return (
    <div className="pointer-events-none fixed bottom-12 right-6 z-50 flex w-[340px] flex-col gap-2">
      {toasts.map((t) =>
        t.kind === "success" ? (
          <div
            key={t.id}
            role="status"
            className="anim-toast-in pointer-events-auto relative overflow-hidden rounded-[14px] border border-[#26262B] bg-[#151517] shadow-[0_24px_60px_-20px_rgba(0,0,0,.9)]"
          >
            <div className="flex gap-3 px-4 py-3.5">
              <span className="flex h-7 w-7 shrink-0 items-center justify-center rounded-lg bg-accent">
                <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="#0A0A0B" strokeWidth="2.6" strokeLinecap="round" aria-hidden="true">
                  <path d="m5 12 5 5 9-10" />
                </svg>
              </span>
              <span className="min-w-0 flex-1">
                <span className="block text-sm font-medium text-text">{t.message}</span>
                {t.sub && <span className="mt-0.5 block font-mono text-[11px] text-text-3">{t.sub}</span>}
              </span>
              <button
                type="button"
                aria-label="Dismiss"
                onClick={() => dismissToast(t.id)}
                className="h-5 shrink-0 text-text-3 transition-colors hover:text-text"
              >
                ×
              </button>
            </div>
            <div className="drain-bar absolute bottom-0 left-0 h-0.5 bg-accent" />
          </div>
        ) : (
          <div
            key={t.id}
            role="alert"
            className="anim-toast-in pointer-events-auto flex gap-3 rounded-[14px] border border-danger-border bg-danger-bg px-4 py-3.5 shadow-[0_24px_60px_-20px_rgba(0,0,0,.9)]"
          >
            <span className="flex h-7 w-7 shrink-0 items-center justify-center rounded-lg bg-danger font-bold text-[#1A0A08]">
              !
            </span>
            <span className="min-w-0 flex-1 text-sm font-medium leading-relaxed text-text">{t.message}</span>
            <button
              type="button"
              aria-label="Dismiss"
              onClick={() => dismissToast(t.id)}
              className="h-5 shrink-0 text-[#D9B4AE] transition-colors hover:text-text"
            >
              ×
            </button>
          </div>
        ),
      )}
    </div>
  );
}
