import { useState } from "react";

import type { AssetType } from "../../api";
import { categoriesFor, useStore } from "../../store";
import { sanitizeName } from "./draft";

interface Props {
  type: AssetType;
  value: string;
  onChange: (value: string) => void;
}

export default function CategoryPicker({ type, value, onChange }: Props) {
  const assets = useStore((s) => s.assets);
  const [adding, setAdding] = useState(false);
  const categories = categoriesFor(assets, type);
  const sanitized = sanitizeName(value);
  const selectValue = adding ? "__new__" : categories.some((c) => c.name === sanitized) ? sanitized : "";

  return (
    <div>
      <label htmlFor="asset-category" className="mb-1.5 block text-xs text-text-2">
        Category
      </label>
      <select
        id="asset-category"
        aria-label="Category"
        value={selectValue}
        onChange={(e) => {
          if (e.target.value === "__new__") {
            setAdding(true);
            onChange("");
          } else {
            setAdding(false);
            onChange(e.target.value);
          }
        }}
        className="h-10 w-full rounded-[10px] border border-line-strong bg-sunken px-3 text-sm text-text"
      >
        <option value="">Select category…</option>
        {categories.map((c) => (
          <option key={c.name} value={c.name}>
            {c.name}
          </option>
        ))}
        <option value="__new__">+ New Category</option>
      </select>
      {adding && (
        <div className="mt-2">
          <label htmlFor="new-category" className="sr-only">
            New Category
          </label>
          <input
            id="new-category"
            aria-label="New Category"
            type="text"
            value={value}
            onChange={(e) => onChange(e.target.value)}
            placeholder="New category"
            className="h-10 w-full rounded-[10px] border border-line-strong bg-sunken px-3 text-sm text-text placeholder:text-text-3"
          />
        </div>
      )}
    </div>
  );
}
