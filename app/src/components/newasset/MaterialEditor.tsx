import { useState, type Dispatch } from "react";

import type { AttrKind } from "../../api";
import AttributeRow from "./AttributeRow";
import Chip, { CHIP_LABELS } from "../ui/Chip";
import { effectiveMaterialName, sanitizeName, type DraftAction, type DraftState } from "./draft";

const KIND_ORDER: AttrKind[] = [
  "BaseColor",
  "Metallic",
  "Roughness",
  "Specular",
  "Normal",
  "Height",
  "AO",
  "Opacity",
  "Emissive",
];

interface Props {
  draft: DraftState;
  dispatch: Dispatch<DraftAction>;
}

function Chevron({ open }: { open: boolean }) {
  return (
    <svg
      width="12"
      height="12"
      viewBox="0 0 24 24"
      fill="none"
      stroke="#A3A3AC"
      strokeWidth="2"
      strokeLinecap="round"
      aria-hidden="true"
      style={{ transform: open ? "rotate(90deg)" : undefined, transition: "transform 200ms" }}
    >
      <path d="m9 6 6 6-6 6" />
    </svg>
  );
}

export default function MaterialEditor({ draft, dispatch }: Props) {
  const [editing, setEditing] = useState<number | null>(null);
  const [editValue, setEditValue] = useState("");
  const [openIndex, setOpenIndex] = useState(0);

  function commitRename(index: number) {
    dispatch({ type: "renameMaterial", index, name: editValue });
    setEditing(null);
  }

  if (draft.materials.length === 0) {
    return <p className="text-xs text-text-3">Select a model file to see its materials.</p>;
  }

  return (
    <div className="flex flex-col gap-2.5">
      {draft.materials.map((material, i) => {
        const displayName = effectiveMaterialName(draft, i);
        const present = new Set(material.attributes.map((a) => a.kind));
        const available = KIND_ORDER.filter((k) => !present.has(k));
        const open = openIndex === i;
        const renamed =
          material.originalName !== "" && sanitizeName(displayName) !== sanitizeName(material.originalName);
        return (
          <div
            key={`${material.originalName}-${i}`}
            className={`overflow-hidden rounded-[14px] border bg-sunken ${
              open ? "border-[#2A2A2F]" : "border-line"
            }`}
          >
            <div
              className={`flex items-center gap-2.5 px-3.5 py-3 ${open ? "border-b border-[#1C1C20]" : ""}`}
            >
              <button
                type="button"
                aria-label={`${open ? "Collapse" : "Expand"} ${displayName}`}
                aria-expanded={open}
                onClick={() => setOpenIndex(open ? -1 : i)}
                className="flex h-6 w-6 shrink-0 items-center justify-center rounded-md text-text-2 transition-colors hover:bg-raised hover:text-text"
              >
                <Chevron open={open} />
              </button>
              {editing === i ? (
                <input
                  type="text"
                  aria-label="Material name"
                  value={editValue}
                  autoFocus
                  onChange={(e) => setEditValue(e.target.value)}
                  onKeyDown={(e) => {
                    if (e.key === "Enter") commitRename(i);
                    if (e.key === "Escape") setEditing(null);
                  }}
                  onBlur={() => commitRename(i)}
                  className="rounded-lg border border-line-strong bg-surface px-2 py-1 text-sm text-text"
                />
              ) : (
                <span
                  onDoubleClick={() => {
                    setEditValue(material.name);
                    setEditing(i);
                  }}
                  title="Double-click to rename"
                  className="cursor-text text-sm font-semibold text-text"
                >
                  {displayName || "Unnamed"}
                </span>
              )}
              {renamed && (
                <span className="font-mono text-[11px] text-text-3">was {material.originalName}</span>
              )}
              {open ? (
                <>
                  <label className="sr-only" htmlFor={`add-attr-${i}`}>
                    Add attribute for {displayName}
                  </label>
                  <select
                    id={`add-attr-${i}`}
                    aria-label={`Add attribute for ${displayName}`}
                    value=""
                    onChange={(e) => {
                      if (e.target.value !== "") {
                        dispatch({ type: "addAttribute", index: i, kind: e.target.value as AttrKind });
                      }
                    }}
                    className="ml-auto h-[30px] rounded-lg border border-dashed border-[#3A3A40] bg-transparent px-2 text-xs text-text-2"
                  >
                    <option value="">+ Attribute</option>
                    {available.map((k) => (
                      <option key={k} value={k}>
                        {k}
                      </option>
                    ))}
                  </select>
                </>
              ) : (
                material.attributes.length > 0 && (
                  <span className="ml-auto flex flex-wrap justify-end gap-1">
                    {material.attributes.map((a) => (
                      <Chip key={a.kind} label={CHIP_LABELS[a.kind]} filled={a.textureSrc !== null} />
                    ))}
                  </span>
                )
              )}
            </div>
            {open && (
              <div className="divide-y divide-[#17171A]">
                {material.attributes.map((attr, j) => (
                  <AttributeRow
                    key={attr.kind}
                    attr={attr}
                    onPatch={(patch) => dispatch({ type: "updateAttribute", index: i, attrIndex: j, patch })}
                    onRemove={() => dispatch({ type: "removeAttribute", index: i, attrIndex: j })}
                  />
                ))}
                {material.attributes.length === 0 && (
                  <p className="px-3.5 py-2.5 text-xs text-text-3">No attributes yet.</p>
                )}
              </div>
            )}
          </div>
        );
      })}
    </div>
  );
}
