# uHub 3D

A desktop asset library for 3D models, materials and HDRIs. It renders path-traced thumbnails, has a `uhub` command line for scripts and AI agents, and exports to DCC apps through plugins. Built with Tauri 2, React and Rust.

## Repository layout

| Folder | Contents |
|---|---|
| `app/` | The desktop app (React front end in `app/src`, Rust back end in `app/src-tauri`) and the `uhub` CLI |
| `plugin/` | DCC plugins that receive exports from the app |

## DCC support

| DCC | Status |
|---|---|
| Blender 5.1 and 5.2 | Supported (Cycles and EEVEE) |
| Unreal Engine (5.7) | Supported |
| Maya (2024+) | Not shipped in this release |

uHub 3D is meant to be universal. The app talks to every DCC through one small localhost protocol, so more DCCs and renderers can be added as plugins without changing the library.

## Plugins

The app and the DCCs talk over a localhost bridge (TCP `127.0.0.1:47913` by default; change it in
*Preferences → DCC Bridge*). The port must be free when the app starts, otherwise the export dock
shows *Bridge offline*. Maya is not shipped in this release.

**Blender** — install the extension zip `plugin/dist/uhub3d-1.0.0.zip` (in Blender:
*Preferences → Get Extensions → Install from Disk…*, or from the command line):

```powershell
& "C:\Program Files\Blender Foundation\Blender 5.1\blender.exe" --command extension install-file -r user_default -e "plugin\dist\uhub3d-1.0.0.zip"
```

Then restart Blender, select an asset in uHub 3D, choose the Blender target and *Cycles* or
*EEVEE* in the export dock, and click *Export*.

**Unreal** — unzip `plugin/dist/uhub3d-unreal.zip` so that `uHub3D/uHub3D.uplugin` sits in
`<YourProject>/Plugins/uHub3D/`. Restart Unreal, choose the *Unreal* target in the export dock, and
click *Export*. Imported content lands under `/Game/uHub3D/` (3D assets, materials, HDRI and the
`_Core` masters).

Tested versions (only these are claimed; other versions are untested):

| DCC | Tested versions |
|---|---|
| Blender | 5.1.2, 5.2.3 LTS |
| Unreal Engine | 5.7.4 |
| Maya | Not shipped in this release |

### Troubleshooting

- **Export dock says *Bridge offline: port 47913 in use*** — another program (often a second
  uHub 3D instance) already holds the bridge port. Close it, or pick a different port in
  *Preferences → DCC Bridge* and restart the DCC so its plugin reconnects.
- **A DCC pill stays grey and the plugin does not connect** — make sure the plugin is installed
  and enabled in the DCC, then restart the DCC (the Blender extension and the Unreal plugin both
  connect on startup and retry every few seconds). If you changed the bridge port in the app,
  set the same port in the Blender add-on preferences and restart the DCC.
- **The Unreal plugin never connects** — it only starts in an editor with a real RHI. Do not
  launch the editor with `-nullrhi`: Unreal runs no Python startup scripts in that mode, so the
  plugin never loads. Start the editor normally.

## Develop

All commands run in `app/`.

Final thumbnails are denoised with Intel Open Image Denoise 2.3.3. Fetch the pinned prebuilt once. Then set `OIDN_DIR` to the value the script prints. On Windows, also put its `bin` folder on `PATH`, so tests and dev runs can load the DLLs.

```powershell
# Windows
cd app
./scripts/fetch_oidn.ps1
$env:OIDN_DIR = "<value printed by the script>"
$env:PATH = "$env:OIDN_DIR\bin;$env:PATH"
```

```sh
# macOS
cd app
./scripts/fetch_oidn.sh
export OIDN_DIR="<value printed by the script>"
```

```sh
npm install
npm run tauri dev
```

To run the tests: `npm test` for the front end, and `cargo test --manifest-path src-tauri/Cargo.toml` for the back end and CLI.

## Build

```sh
cd app
npm run tauri build
```

Windows installers (MSI and NSIS) are written to `app/src-tauri/target/release/bundle/`.

## macOS (unsigned build)

The CI workflow (`.github/workflows/build.yml`) builds a universal macOS bundle on `macos-latest`. It runs for `v*` tags or when started by hand from the Actions tab. The build is not signed yet, so Gatekeeper refuses the first launch. After installing:

1. Move `uHub 3D.app` to `/Applications`.
2. Clear the quarantine flag:

   ```sh
   xattr -cr "/Applications/uHub 3D.app"
   ```

3. Open the app normally.

Signing and notarization can be turned on later by filling in the commented `APPLE_*` secrets in the workflow.

## Command line

The installer also ships `uhub`, a CLI that can build a whole library without the GUI. `uhub guide` prints its full guide (also in `app/docs/cli/AGENT_GUIDE.md`).

## License

uHub 3D is free software under the GNU General Public License v3.0 or later. See [LICENSE](LICENSE). Bundled third-party components keep their own licenses, listed in [app/THIRD_PARTY_NOTICES.md](app/THIRD_PARTY_NOTICES.md).
