# AutoCreamAPI

[![Build](https://github.com/Mooshieblob1/AutoCreamAPI/actions/workflows/build.yml/badge.svg)](https://github.com/Mooshieblob1/AutoCreamAPI/actions/workflows/build.yml)

A desktop application that automatically sets up [CreamAPI](https://cs.rin.ru/forum/viewtopic.php?f=29&t=70576) for Steam games. Built with **Rust/Tauri 2** and **Svelte 5**, replacing the original .NET-based AutoCreamAPI.

## Features

- **Automatic Game Discovery** — Scans all Steam library folders by parsing local ACF manifest files. No API key required.
- **Dual-Source DLC Fetching** — Retrieves DLC lists from both the Steam Store API and steamcmd.net, including hidden/unlisted DLCs.
- **One-Click Apply/Remove** — Backs up original Steam API DLLs, installs CreamAPI replacements, and generates `cream_api.ini` configuration automatically.
- **x86 & x64 Support** — Detects and handles both 32-bit and 64-bit Steam API DLLs per game.
- **Offline Mode** — Optional setting to force CreamAPI into offline mode.
- **Extra Protection** — Optional flag for games with additional DRM checks.
- **Applied Status Detection** — Shows which games already have CreamAPI installed by detecting backup DLLs.
- **Hidden DLC Badges** — DLCs sourced from steamcmd.net (not visible on the Steam Store) are tagged as "Hidden".
- **Search & Filter** — Quickly find games in large libraries.

## Screenshots

*Coming soon*

## Requirements

- **Windows 10/11** (64-bit)
- **Steam** installed with at least one game library
- **CreamAPI DLLs** — The app ships with bundled CreamAPI DLLs (`steam_api.dll` and `steam_api64.dll`) in its resources

## Installation

### Pre-built Installers

Download the latest installer from the [Actions](https://github.com/Mooshieblob1/AutoCreamAPI/actions) artifacts or [Releases](https://github.com/Mooshieblob1/AutoCreamAPI/releases):

- **`AutoCreamAPI_x.x.x_x64-setup.exe`** — NSIS installer (recommended)
- **`AutoCreamAPI_x.x.x_x64_en-US.msi`** — MSI installer

### Build from Source

#### Prerequisites

- [Rust](https://rustup.rs/) (stable, ≥1.77.2)
- [Node.js](https://nodejs.org/) (≥22)
- [pnpm](https://pnpm.io/) (≥10)

#### Steps

```bash
git clone https://github.com/Mooshieblob1/AutoCreamAPI.git
cd AutoCreamAPI
pnpm install
pnpm tauri build
```

The built executable and installers will be in `src-tauri/target/release/` and `src-tauri/target/release/bundle/`.

For development with hot-reload:

```bash
pnpm tauri dev
```

## Usage

1. **Launch AutoCreamAPI** — The app automatically scans your Steam libraries for installed games.
2. **Select a game** from the left panel.
3. **Review the DLC list** — All available DLCs are fetched and pre-selected. Deselect any you don't want.
4. **Configure settings** (optional):
   - **Offline Mode** — Forces CreamAPI to work without a network connection.
   - **Extra Protection** — Enables additional compatibility for certain games.
5. **Click "Apply CreamAPI"** — The app will:
   - Back up the original `steam_api.dll` / `steam_api64.dll`
   - Rename originals to `steam_api_o.dll` / `steam_api64_o.dll`
   - Write the CreamAPI DLLs in their place
   - Generate a `cream_api.ini` with your selected DLCs
6. **To revert**, select the game and click **"Remove CreamAPI"** — originals are restored from backups.

## How It Works

### Game Discovery

The app reads the Windows registry (`HKCU\Software\Valve\Steam`) to find Steam's installation path, then parses `libraryfolders.vdf` to locate all library folders. Each library's `steamapps/` directory is scanned for `.acf` manifest files which contain game metadata (app ID, name, install directory). Games are checked for the presence of `steam_api.dll` and/or `steam_api64.dll` to confirm they use the Steam API.

### DLC Fetching

Two sources are queried concurrently:

| Source | Endpoint | What it provides |
|--------|----------|-----------------|
| **Steam Store API** | `store.steampowered.com/api/appdetails` | Official DLC list with names |
| **SteamCMD Web API** | `api.steamcmd.net/v1/info` | Hidden/unlisted DLCs via `extended.listofdlc` and depot metadata |

Results are merged and deduplicated. DLCs only found via SteamCMD are marked as "Hidden" in the UI. Placeholder names are resolved with follow-up Store API calls.

### CreamAPI Application

When applied, the app:

1. Reads the bundled CreamAPI DLLs from Tauri's resource directory
2. Creates a `.backup` copy of the original Steam API DLL
3. Renames the original to `*_o.dll` (required by CreamAPI as the "original API" reference)
4. Writes the CreamAPI DLL in place of the original
5. Generates `cream_api.ini` with app ID, DLC list, and settings

## Tech Stack

| Layer | Technology |
|-------|-----------|
| **Framework** | [Tauri 2](https://tauri.app/) |
| **Backend** | Rust (edition 2021) |
| **Frontend** | [Svelte 5](https://svelte.dev/) + TypeScript |
| **Build Tool** | [Vite 8](https://vite.dev/) |
| **HTTP Client** | reqwest 0.12 |
| **VDF Parsing** | keyvalues-serde 0.2 |
| **Registry** | winreg 0.55 |
| **CI/CD** | GitHub Actions (Windows) |
| **Package Manager** | pnpm |

## Project Structure

```
AutoCreamAPI/
├── src/                        # Frontend (Svelte)
│   ├── App.svelte              # Main UI component
│   ├── main.ts                 # Entry point
│   └── styles.css              # Dark theme styles
├── src-tauri/                  # Backend (Rust/Tauri)
│   ├── src/
│   │   ├── lib.rs              # Tauri command handlers
│   │   ├── steam.rs            # Steam game discovery
│   │   ├── dlc.rs              # DLC fetching (dual-source)
│   │   └── creamapi.rs         # DLL management & config generation
│   ├── resources/              # Bundled CreamAPI DLLs
│   │   ├── steam_api.dll
│   │   └── steam_api64.dll
│   ├── Cargo.toml
│   └── tauri.conf.json
├── .github/workflows/
│   └── build.yml               # CI: Windows build + artifact upload
├── package.json
└── vite.config.ts
```

## Background

This project is a ground-up rewrite of the original [AutoCreamAPI](https://github.com/MoebiusZero/AutoCreamAPI), which was a .NET Core 3.1 WPF application. The original stopped working due to:

1. **.NET Core 3.1 reaching end-of-life** — The compiled binary would no longer launch on systems without the deprecated runtime.
2. **Steam API deprecation** — The `ISteamApps/GetAppList/v2` endpoint used for game discovery was shut down (returns 404).
3. **Stale CreamAPI DLLs** — The bundled DLLs were outdated and incompatible with newer Steam client updates.

This rewrite addresses all three issues by using Rust (no runtime dependency), local file-based game discovery (no Steam web API needed), and bundling current CreamAPI DLLs as Tauri resources.

## License

GPL-3.0 — see [LICENSE](LICENSE) for details.

## Disclaimer

This software is provided for educational and personal use. The authors are not responsible for any misuse. Use at your own risk and in accordance with applicable terms of service.
