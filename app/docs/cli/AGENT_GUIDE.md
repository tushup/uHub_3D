# uHub 3D — Agent Guide

You are building a 3D asset library with the `uhub` CLI. Everything you add appears in the uHub 3D desktop app (it rescans the library when its window gains focus — never edit library folders by hand).

## Quick start

```
uhub guide                      # this file
uhub schema                     # the manifest JSON Schema
uhub --json info                # library root + asset counts
uhub inspect chair.fbx          # look at a model before adding it
uhub add manifest.json --dry-run   # validate, print the exact plan, write nothing
uhub add manifest.json          # add (thumbnails are path-traced, 10 s per asset)
uhub --json list --type model   # what is in the library
```

Global flags: `--library <dir>` picks the library root (else env `UHUB_LIBRARY`, else the app's settings, else `~/Documents/uHub3D/Database`). `--json` makes stdout pure JSON; progress and logs go to stderr.

## The workflow: inspect → manifest → dry-run → add

1. **Inspect** every source file first. `uhub inspect file.fbx` prints format, meshes, vertices, bounds, and the materials *as named inside the file*, with the textures it found. `uhub inspect file.hdr` prints the size. Those internal material names matter (see Binding).
2. **Write a manifest** (below). Relative paths resolve from the manifest file's own folder, so put the manifest next to the assets or reference them relatively. Use `-` as the manifest path to read stdin (paths then resolve from the current directory).
3. **Dry-run**: `uhub add manifest.json --dry-run` validates names, formats and file existence, and prints the exact folder and file list. It writes nothing. Fix every error here.
4. **Add**: `uhub add manifest.json`. 3D and Material assets are path-traced from their `view` for 10 s at 512×512 (RGBA PNG). Use `--seconds S` or `--samples N` to change that. Final renders are denoised with OIDN; pass `--no-denoise` to `add` or `render` to skip it. One manifest may be an **array** of drafts (a batch): items run in order, one failure does not stop the rest, output is one JSON result per item, and the exit code is the highest among items.

## Naming rules

- `name` and `category` are required. They are sanitized: trimmed, spaces become `_`, characters outside `[A-Za-z0-9_]` are dropped. The result must not be empty.
- A name must be unique inside its type + category; a second add of the same name exits with code 4.
- Folders are `<Library>/<3D|Materials|HDRI>/<Category>/<Name>/`. Assets are referenced by `id` (printed by `add`) or by that `<Type>/<Category>/<Name>` path in `show`, `render` and `delete`.

## Manifest reference

A manifest is one asset draft, or an array of them. All of `assetType`, `name`, `category`, `tags`, `materials`, `rotation`, `intensity` are required (use `[]`, `0`, `1` when irrelevant). `uhub schema` prints the full JSON Schema.

### Asset types and files

| `assetType` | Source field | Formats | Thumbnail |
|---|---|---|---|
| `model` | `modelSrc` | `.fbx`, `.obj` | Path-traced render of `view` (512×512 RGBA) |
| `material` | `materials` (one) | textures `.png .jpg .jpeg .tga` | Path-traced sphere (UVs tiled 2×1) |
| `hdri` | `hdriSrc` | `.hdr` | 1024×512 equirect, AgX-tonemapped, with `rotation`/`intensity` |

`thumbnailSrc` (CLI only) replaces the render with a fitted image — prefer leaving it out so thumbnails stay re-renderable from the stored `view`.

### Materials and binding

Each mesh group in a model uses the draft material whose `originalName` equals the material name inside the model file (that is what `inspect` lists). If the manifest has **exactly one** material, it applies to every group (and is the only possibility for `material` assets). Groups without a match get flat defaults (baseColor 0.8, roughness 0.5, metallic 0, specular 0.5, opacity 1). The `name` field is the display name stored in the library.

### Attributes

| `kind` | `value` | Texture | Notes |
|---|---|---|---|
| `BaseColor` | `[r,g,b]` linear | sRGB decode | Albedo. |
| `Metallic` | `0..1` | linear (red channel) | |
| `Roughness` | `0..1` | linear (red channel) | Clamped to ≥ 0.02 in the renderer. |
| `Specular` | `0..1` | linear (red channel) | Dielectric F0 = 0.08 · specular. |
| `Opacity` | `0..1` | linear (red channel) | Stochastic transparency. |
| `Emissive` | `[r,g,b]` linear | sRGB decode | Emission strength 1. |
| `Normal` | — | linear | Tangent-space. `strength` scales it. `normalY`: `"OpenGL"` (green up, default) or `"DirectX"` (green down — the renderer flips it). |
| `Height` | scalar | — | Stored, not displaced or rendered. |
| `AO` | — | linear | Stored, ignored by the path tracer (real occlusion is traced). |

Give **either** `value` **or** `textureSrc` per attribute: when a texture is present it replaces the value at each UV. Textures are stored converted to the library's naming scheme; the originals are copied, never converted between formats.

**sRGB rule:** only `BaseColor` and `Emissive` textures are colour-managed (sRGB → linear). Everything else is read as linear data. Do not pre-convert.

## Complete examples

A 3D model — `originalName` values come from `uhub inspect chair.fbx`:

```json
{
  "assetType": "model",
  "name": "Chair",
  "category": "Furniture",
  "tags": ["wood"],
  "modelSrc": "chair.fbx",
  "materials": [
    {
      "originalName": "SeatMat",
      "name": "Seat",
      "attributes": [
        { "kind": "BaseColor", "textureSrc": "chair_seat_basecolor.png" },
        { "kind": "Roughness", "value": 0.6 }
      ]
    },
    {
      "originalName": "LegMat",
      "name": "Legs",
      "attributes": [
        { "kind": "BaseColor", "value": [0.2, 0.2, 0.22] },
        { "kind": "Metallic", "value": 0.9 },
        { "kind": "Roughness", "value": 0.35 }
      ]
    }
  ],
  "rotation": 0,
  "intensity": 1,
  "view": { "yaw": -30, "pitch": 15, "zoom": 1 }
}
```

A standalone material:

```json
{
  "assetType": "material",
  "name": "Oak",
  "category": "Wood",
  "tags": ["floor"],
  "materials": [
    {
      "originalName": "Oak",
      "name": "Oak",
      "attributes": [
        { "kind": "BaseColor", "textureSrc": "oak_basecolor.png" },
        { "kind": "Roughness", "value": 0.55 },
        { "kind": "Normal", "textureSrc": "oak_normal.png", "strength": 1.0, "normalY": "OpenGL" }
      ]
    }
  ],
  "rotation": 0,
  "intensity": 1
}
```

An HDRI:

```json
{
  "assetType": "hdri",
  "name": "Sky",
  "category": "Skies",
  "tags": [],
  "hdriSrc": "sky.hdr",
  "materials": [],
  "rotation": 45,
  "intensity": 1.2
}
```

Note: the material example references `oak_normal.png`, which is optional — remove that attribute if you do not have the file (a `--dry-run` checks that every referenced file exists). The examples above are verified with `uhub add --dry-run` against files named `chair.fbx`, `chair_seat_basecolor.png`, `oak_basecolor.png`, `oak_normal.png` and `sky.hdr` placed next to the manifest.

## Texture filename conventions

When a model ships loose textures, map suffixes to attributes:

| Suffixes | `kind` | Extras |
|---|---|---|
| `diff`, `albedo`, `col`, `basecolor`, `base_color` | `BaseColor` | sRGB |
| `nor_gl`, `normal_gl`, `nrm_gl` | `Normal` | `normalY: "OpenGL"` |
| `nor_dx`, `normal_dx`, `nrm_dx` | `Normal` | `normalY: "DirectX"` |
| `rough`, `roughness` | `Roughness` | |
| `metal`, `metallic`, `metalness` | `Metallic` | |
| `ao`, `occlusion` | `AO` | stored, not rendered |
| `disp`, `height`, `bump` | `Height` | stored, not rendered |
| `opacity`, `alpha` | `Opacity` | |
| `emit`, `emissive`, `emission` | `Emissive` | sRGB |

Packed maps (for example `arm` = AO/roughness/metallic in one image) are **not supported**: skip them instead of guessing channels.

## Views and re-rendering

`view` is `{ "yaw": -30, "pitch": 15, "zoom": 1 }` by default; drag semantics are yaw/pitch in degrees, zoom 0.5–4. It is stored in `asset.json`. Re-render any 3D/Material thumbnail with `uhub render <ref> [--yaw Y] [--pitch P] [--zoom Z] [--seconds S] [--samples N] [--no-denoise]` (only the given components change). HDRI thumbnails regenerate from the stored rotation/intensity.

## Other commands

- `uhub info` — library root, version, counts.
- `uhub categories [--type model|material|hdri]`
- `uhub list [--type T] [--category C] [--search S]`
- `uhub show <ref>` — full `asset.json` + folder.
- `uhub delete <ref> --yes` — moves the asset folder to the OS trash. Without `--yes` it refuses (exit 2). There is no undo through the CLI.

## Errors and exit codes

With `--json`, failures look like `{"ok":false,"error":{"code":"name_exists","message":"..."}}`; codes are stable snake_case: `invalid_name`, `name_exists`, `unsupported_format`, `missing_file`, `not_found`, `render_failed`, `io`.

| Exit | Meaning |
|---|---|
| 0 | ok |
| 1 | other error (I/O) |
| 2 | usage (bad flags, `delete` without `--yes`) |
| 3 | invalid input (validation, unsupported format, missing file) |
| 4 | name exists |
| 5 | asset not found |
| 6 | render failed or cancelled |

A batch exits with the highest code among its items.
