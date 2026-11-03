import { open } from "@tauri-apps/plugin-dialog";
import { useEffect, useState } from "react";

import { cliPath, getSettings, listAssets, onPreferences, setSettings } from "../api";
import { useStore } from "../store";
import Button from "./ui/Button";

function errMessage(e: unknown): string {
  return e instanceof Error ? e.message : String(e);
}

type Section = "library" | "bridge" | "about";

export default function PreferencesDialog() {
  const [session, setSession] = useState(0);
  const prefsNonce = useStore((s) => s.prefsNonce);

  useEffect(() => {
    if (prefsNonce > 0) setSession((n) => n + 1);
  }, [prefsNonce]);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let cancelled = false;
    onPreferences(() => setSession((n) => n + 1))
      .then((fn) => {
        if (cancelled) fn();
        else unlisten = fn;
      })
      .catch(() => {});
    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, []);

  if (session === 0) return null;
  return <PreferencesSession key={session} onClose={() => setSession(0)} />;
}

function PreferencesSession({ onClose }: { onClose: () => void }) {
  const [section, setSection] = useState<Section>("library");
  const [libraryRoot, setLibraryRoot] = useState("");
  const [port, setPort] = useState(47913);
  const [originalPort, setOriginalPort] = useState(47913);
  const [loaded, setLoaded] = useState(false);
  const [cli, setCli] = useState("");
  const [copied, setCopied] = useState(false);
  const [saving, setSaving] = useState(false);
  const [saveError, setSaveError] = useState<string | null>(null);
  const setAssets = useStore((s) => s.setAssets);
  const setStoredSettings = useStore((s) => s.setSettings);
  const toast = useStore((s) => s.toast);
  const assetCount = useStore((s) => s.assets.length);
  const categoryCount = useStore(
    (s) => new Set(s.assets.map((a) => `${a.asset.type}/${a.asset.category}`)).size,
  );

  useEffect(() => {
    getSettings()
      .then((s) => {
        setLibraryRoot(s.libraryRoot);
        setPort(s.port);
        setOriginalPort(s.port);
        setLoaded(true);
      })
      .catch((e: unknown) => setSaveError(errMessage(e)));
  }, []);

  useEffect(() => {
    if (section !== "about") return;
    cliPath()
      .then(setCli)
      .catch(() => setCli(""));
  }, [section]);

  async function copyCliPath() {
    try {
      await navigator.clipboard.writeText(cli);
      setCopied(true);
    } catch {
      // Clipboard unavailable; the path is still shown to copy by hand.
    }
  }

  useEffect(() => {
    function onKey(e: globalThis.KeyboardEvent) {
      if (e.key === "Escape") onClose();
    }
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);

  const portValid = Number.isInteger(port) && port >= 1024 && port <= 65535;
  const valid = loaded && libraryRoot.trim() !== "" && portValid;

  async function browse() {
    const picked = await open({ directory: true, multiple: false });
    if (typeof picked === "string") setLibraryRoot(picked);
  }

  async function onSave() {
    setSaving(true);
    setSaveError(null);
    try {
      const settings = { libraryRoot, port };
      await setSettings(settings);
      setStoredSettings(settings);
      setAssets(await listAssets());
      if (port !== originalPort) toast("success", "Port change applies after restart");
      onClose();
    } catch (e) {
      setSaveError(errMessage(e));
    } finally {
      setSaving(false);
    }
  }

  const navClass = (active: boolean) =>
    `flex h-9 w-full items-center rounded-[9px] px-3 text-left text-[13px] font-medium transition-colors duration-150 ${
      active ? "bg-raised text-text" : "text-text-2 hover:text-text"
    }`;

  const portField = (
    <div>
      <label htmlFor="pref-port" className="mb-1 block text-[13px] font-medium text-text">
        Bridge port
      </label>
      <p className="mb-2.5 text-xs leading-relaxed text-text-2">
        DCC plugins connect on this local port. Applies after restart.
      </p>
      <input
        id="pref-port"
        aria-label="Bridge Port"
        type="number"
        min={1024}
        max={65535}
        step={1}
        value={port}
        onChange={(e) => setPort(Number(e.target.value))}
        className="h-10 w-[140px] rounded-[10px] border border-line-strong bg-sunken px-3 font-mono text-[13px] text-text outline-none transition-colors focus:border-accent"
      />
      {!portValid && loaded && (
        <p className="mt-1.5 text-xs text-danger-text">Port must be between 1024 and 65535.</p>
      )}
    </div>
  );

  return (
    <div className="fixed inset-0 z-40 flex items-center justify-center bg-[rgba(5,5,6,.72)] p-6 backdrop-blur-[6px]">
      <section
        role="dialog"
        aria-modal="true"
        aria-label="Preferences"
        className="anim-pop flex w-[760px] max-w-full overflow-hidden rounded-[20px] border border-line-strong bg-surface shadow-[0_40px_120px_-30px_rgba(0,0,0,.9)]"
      >
        <nav aria-label="Preferences sections" className="flex w-[220px] shrink-0 flex-col gap-1 border-r border-[#1C1C20] bg-[#0E0E10] px-3 py-5">
          <h2 className="mx-3 mb-3.5 text-base font-semibold tracking-[-0.01em] text-text">Preferences</h2>
          <button type="button" className={navClass(section === "library")} onClick={() => setSection("library")}>
            Library
          </button>
          <button type="button" className={navClass(section === "bridge")} onClick={() => setSection("bridge")}>
            Bridge
          </button>
          <button type="button" className={navClass(section === "about")} onClick={() => setSection("about")}>
            About
          </button>
        </nav>

        <div className="flex min-w-0 flex-1 flex-col gap-6 px-7 py-6">
          {section === "library" && (
            <>
              <div>
                <label htmlFor="pref-library-root" className="mb-1 block text-[13px] font-medium text-text">
                  Library location
                </label>
                <p className="mb-2.5 text-xs leading-relaxed text-text-2">
                  Where uHub stores assets. Point it at a NAS share later to share a library with your team.
                </p>
                <div className="flex gap-2">
                  <input
                    id="pref-library-root"
                    aria-label="Library Root"
                    type="text"
                    value={libraryRoot}
                    onChange={(e) => setLibraryRoot(e.target.value)}
                    className="h-10 min-w-0 flex-1 rounded-[10px] border border-line-strong bg-sunken px-3 font-mono text-[13px] text-text outline-none transition-colors focus:border-accent"
                  />
                  <Button variant="secondary" className="shrink-0" onClick={() => void browse()}>
                    Browse…
                  </Button>
                </div>
                <div className="mt-2.5 flex gap-[18px] font-mono text-[11px] text-text-2">
                  <span>
                    <span className="text-text">{assetCount}</span> ASSETS
                  </span>
                  <span>
                    <span className="text-text">{categoryCount}</span> CATEGORIES
                  </span>
                </div>
              </div>
              {portField}
            </>
          )}
          {section === "bridge" && portField}
          {section === "about" && (
            <div>
              <p className="text-[13px] font-medium text-text">uHub 3D</p>
              <p className="mt-1 font-mono text-[11px] text-text-3">0.1.0 · Studio Dark</p>
              <p className="mt-5 text-[13px] font-medium text-text">Agent CLI</p>
              <p className="mt-1 text-xs text-text-2">
                The <span className="font-mono">uhub</span> command-line tool ships next to the
                app. Run <span className="font-mono">uhub guide</span> to get started.
              </p>
              <div className="mt-2 flex items-center gap-2">
                <code className="min-w-0 flex-1 truncate rounded-lg border border-line bg-sunken px-2.5 py-2 font-mono text-[11px] text-text">
                  {cli}
                </code>
                <Button variant="secondary" className="shrink-0" onClick={() => void copyCliPath()}>
                  {copied ? "Copied" : "Copy"}
                </Button>
              </div>
            </div>
          )}

          <div className="mt-auto flex items-center justify-end gap-2 pt-2">
            {saveError && <p className="mr-auto min-w-0 truncate text-xs text-danger-text">{saveError}</p>}
            <Button variant="ghost" onClick={onClose}>
              Cancel
            </Button>
            <button
              type="button"
              disabled={!valid || saving}
              onClick={() => void onSave()}
              className="inline-flex h-[38px] items-center justify-center rounded-[10px] bg-accent px-4 text-[13px] font-medium text-accent-ink transition-all duration-150 hover:bg-[#E4FF6A] active:scale-[.97] disabled:pointer-events-none disabled:opacity-40"
            >
              {saving ? "Saving…" : "Save"}
            </button>
          </div>
        </div>
      </section>
    </div>
  );
}
