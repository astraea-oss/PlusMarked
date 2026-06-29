# Building MarkdownPlus

MarkdownPlus is built with Rust, Tauri 2, SvelteKit, and npm. The app is designed to run locally and keep app-owned data in `MarkdownPlusData/` beside the executable unless `MARKDOWNPLUS_PORTABLE_HOME` is set.

## Common Requirements

- Rust stable.
- Node.js 22 or newer.
- npm.

Install frontend dependencies from the desktop package:

```bash
npm --prefix apps/desktop ci
```

Run validation:

```bash
npm --prefix apps/desktop run check
npm --prefix apps/desktop run build
cargo check --workspace
cargo test --workspace
```

Run the desktop app in development:

```bash
npm --prefix apps/desktop run tauri dev
```

Build a desktop bundle:

```bash
npm --prefix apps/desktop run tauri build
```

Platform-specific Tauri config files control installer targets:

- Linux: `.deb` and `.rpm`.
- Windows: NSIS and MSI.

## Linux

Install Tauri's WebKit and bundling dependencies for your distribution. On Ubuntu/Debian:

```bash
sudo apt-get update
sudo apt-get install -y \
  libwebkit2gtk-4.1-dev \
  libappindicator3-dev \
  librsvg2-dev \
  patchelf
```

MarkdownPlus routes Linux runtime data into:

```text
MarkdownPlusData/
  runtime/
    config/
    data/
    cache/
    temp/
```

It does this before Tauri starts by setting `XDG_CONFIG_HOME`, `XDG_DATA_HOME`, `XDG_CACHE_HOME`, and `TMPDIR`.

The Linux bundle target intentionally skips AppImage for now because AppImage bundling depends on `linuxdeploy` behavior that varies by build host. `.deb` and `.rpm` are the current supported Linux package outputs.

## Windows

Use a native Windows shell for Windows builds, not WSL, when producing Windows bundles.

Install:

- Microsoft C++ Build Tools with the MSVC toolchain.
- Rust stable for the MSVC target.
- Node.js 22 or newer.
- Microsoft Edge WebView2 Runtime.
- WiX Toolset if building MSI installers locally.

MarkdownPlus routes Windows runtime data into:

```text
MarkdownPlusData\
  runtime\
    config\
    data\
    cache\
    temp\
```

It does this before Tauri starts by setting `APPDATA`, `LOCALAPPDATA`, `TEMP`, and `TMP`.

## Portable Data Override

Set `MARKDOWNPLUS_PORTABLE_HOME` to place app-owned settings and runtime data somewhere explicit:

Linux:

```bash
MARKDOWNPLUS_PORTABLE_HOME="$PWD/MarkdownPlusData" npm --prefix apps/desktop run tauri dev
```

Windows PowerShell:

```powershell
$env:MARKDOWNPLUS_PORTABLE_HOME = "$PWD\MarkdownPlusData"
npm --prefix apps/desktop run tauri dev
```

User-selected workspaces can live anywhere. Workspace paths are normalized by the Rust workspace layer after the directory is created.
