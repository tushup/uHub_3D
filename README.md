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
| Blender (4.2+) | In development |
| Unreal Engine (5.3+) | In development |
| Maya (2024+) | Planned |

uHub 3D is meant to be universal. The app talks to every DCC through one small localhost protocol, so more DCCs and renderers can be added as plugins without changing the library.

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
