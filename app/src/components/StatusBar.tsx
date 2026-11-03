import { useStore } from "../store";
import { dccDisplayName, majorMinor } from "./ExportPanel";
import { TABS } from "./TopBar";
import { pad2 } from "./sphere";

export default function StatusBar() {
  const tab = useStore((s) => s.tab);
  const connections = useStore((s) => s.connections);
  const count = useStore((s) => s.assets.filter((a) => a.asset.type === s.tab).length);
  const tabLabel = TABS.find((t) => t.type === tab)?.label ?? tab;

  return (
    <footer className="flex h-[30px] items-center justify-between border-t border-divider bg-bg px-4 font-mono text-[11px] text-text-3">
      <span className="flex min-w-0 items-center gap-4">
        <span className="flex items-center gap-2">
          <span className="pulse-dot inline-block h-1.5 w-1.5 rounded-full bg-accent" />
          Library ready
        </span>
        {connections.map((conn) => (
          <span key={conn.id} className="flex items-center gap-2">
            <span className="pulse-dot inline-block h-1.5 w-1.5 rounded-full bg-accent" />
            {dccDisplayName(conn.dcc)} {majorMinor(conn.dccVersion)} (pid {conn.pid})
          </span>
        ))}
      </span>
      <span>
        {pad2(count)} assets · {tabLabel}
      </span>
    </footer>
  );
}
