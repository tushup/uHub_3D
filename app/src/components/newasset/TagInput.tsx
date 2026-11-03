import { useState, type KeyboardEvent } from "react";

interface Props {
  tags: string[];
  onChange: (tags: string[]) => void;
}

export default function TagInput({ tags, onChange }: Props) {
  const [input, setInput] = useState("");

  function addTag(raw: string) {
    const tag = raw.trim();
    if (tag === "" || tags.includes(tag)) return;
    onChange([...tags, tag]);
  }

  function onKeyDown(e: KeyboardEvent<HTMLInputElement>) {
    if (e.key === "Enter" || e.key === ",") {
      e.preventDefault();
      addTag(input);
      setInput("");
    } else if (e.key === "Backspace" && input === "" && tags.length > 0) {
      onChange(tags.slice(0, -1));
    }
  }

  return (
    <div>
      <label htmlFor="asset-tags" className="mb-1.5 block text-xs text-text-2">
        Tags
      </label>
      <div className="flex min-h-10 flex-wrap items-center gap-1.5 rounded-[10px] border border-line-strong bg-sunken px-2 py-1.5">
        {tags.map((t) => (
          <span
            key={t}
            className="flex items-center gap-1 rounded-full bg-[#1C1C20] px-[9px] py-[3px] font-mono text-[11px] text-text"
          >
            #{t}
            <button
              type="button"
              aria-label={`Remove tag ${t}`}
              onClick={() => onChange(tags.filter((x) => x !== t))}
              className="text-text-3 transition-colors hover:text-text"
            >
              ×
            </button>
          </span>
        ))}
        <input
          id="asset-tags"
          aria-label="Tags"
          type="text"
          value={input}
          onChange={(e) => {
            if (e.target.value.endsWith(",")) {
              addTag(e.target.value.slice(0, -1));
              setInput("");
            } else {
              setInput(e.target.value);
            }
          }}
          onKeyDown={onKeyDown}
          onBlur={() => {
            addTag(input);
            setInput("");
          }}
          placeholder="Add tag…"
          className="min-w-20 flex-1 bg-transparent px-1 py-0.5 text-[13px] text-text outline-none placeholder:text-text-3"
        />
      </div>
    </div>
  );
}
