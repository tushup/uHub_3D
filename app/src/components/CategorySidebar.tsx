import { categoriesFor, useStore } from "../store";
import Eyebrow from "./ui/Eyebrow";
import { pad2 } from "./sphere";

export default function CategorySidebar() {
  const tab = useStore((s) => s.tab);
  const assets = useStore((s) => s.assets);
  const category = useStore((s) => s.category);
  const setCategory = useStore((s) => s.setCategory);
  const settings = useStore((s) => s.settings);

  const tabCount = assets.filter((a) => a.asset.type === tab).length;
  const categories = categoriesFor(assets, tab);

  const itemClass = (active: boolean) =>
    `flex h-[34px] w-full items-center justify-between rounded-lg px-3 text-[13px] font-medium transition-colors duration-200 ${
      active ? "bg-raised text-text" : "text-text-2 hover:bg-[#151518] hover:text-text"
    }`;

  return (
    <nav aria-label="Categories" className="flex h-full flex-col gap-1 overflow-y-auto px-3 py-5">
      <Eyebrow className="px-3 pb-2.5">Categories</Eyebrow>
      <button type="button" className={itemClass(category === null)} onClick={() => setCategory(null)}>
        <span>All</span>
        <span className="font-mono text-[11px] text-text-3">{pad2(tabCount)}</span>
      </button>
      {categories.map((c) => (
        <button
          key={c.name}
          type="button"
          className={itemClass(category === c.name)}
          onClick={() => setCategory(c.name)}
        >
          <span className="truncate">{c.name}</span>
          <span className="font-mono text-[11px] text-text-3">{pad2(c.count)}</span>
        </button>
      ))}
      <div className="mt-auto border-t border-divider px-3 pt-3.5">
        <Eyebrow className="mb-2">Library</Eyebrow>
        <div className="break-all font-mono text-[11px] leading-relaxed text-text-2">
          {settings?.libraryRoot ?? "…"}
        </div>
      </div>
    </nav>
  );
}
