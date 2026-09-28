# AVDL - The Ultimate Desktop Video Browser & Downloader

Language: [简体中文](../README.md) | [繁體中文](README.zh-TW.md) | **English** | [日本語](README.ja.md)

Discord: [Join our Discord Community](https://discord.gg/GACc7HhHY)

> **Ultra-Lightweight · Instant Streaming**  
> AVDL is a modern, high-performance desktop client crafted for media enthusiasts. Breaking away from tedious web pagination and loading pauses, AVDL gathers multi-source content onto a single fluid, boundless canvas. Experience unprecedented browsing, previews, inline playback, and one-click downloads. Portable builds are only **~10 MB**, launching in milliseconds with minimal system footprint. **Fullscreen usage recommended.**

---

## ✨ Key Features

### 🌌 Infinite Exploration Canvas

- **No More Traditional Pagination**: Say goodbye to rigid grids and pagination buttons. All videos are rendered seamlessly across an expansive, continuous canvas.
- **Fluid Zoom & Pan**: Freely zoom in and out without steps using the mouse wheel, and drag-to-pan like navigating a panoramic map.
- **Dynamic Viewport Culling**: Ultra-high frame rate transitions ensure buttery-smooth responsiveness regardless of how many items are loaded.

![Preview](./img/preview.webp)

### 🎬 Instant Online Streaming (Built-in High-Speed Player)

- **No Waiting for Downloads**: Forget the hassle of waiting for a file to download before previewing. Click any card to launch immediate, immersive streaming.
- **Full Player Capabilities**: Adaptive quality switching, instant scrubbing, full-screen viewing, and an immersive cinema dark mode.

### 🔍 Deep Metadata & Contextual Detail Popovers

- **Quick Inspection on Right-Click**: Right-click any card to open a contextual inspector popover.
- **Comprehensive Metadata**: Instant access to video IDs, performers, category tags, and high-resolution posters.

### 📥 High-Speed Resumable Download Engine

- **One-Click Enqueue**: Hover over a card and click the top-left download icon to queue the task instantly.
- **Resilient Resumption**: Multi-task high-speed parallel downloads with automatic segment tracking. If your network disconnects or the app restarts, download progress is preserved—never redownload from scratch.

### 🌐 Multi-Site Aggregation & Smart Proxy Routing

- **Seamless Multi-Source Switching**: Switch between supported content providers (such as JableTV, MissAV, SupJav) with one click.
- **Comprehensive Filters & Sorting**: Explore categories, ranking charts, recent releases, and highest popularity alongside instant keyword search.
- **System Proxy Awareness**: Automatically discovers and routes traffic through system proxies (Clash Verge, etc.) with custom endpoint configuration support for reliable cross-regional access.

---

## 🖱️ Interaction & Shortcut Guide

Fully mouse and trackpad controllable: Middle-click drag to pan, right-click for details, left-click to stream.

| Action | Control Method | Description |
| :--- | :--- | :--- |
| **Pan Canvas** | Hold Middle Mouse Button & Drag / Two-Finger Trackpad Swipe | Freely move the viewport across the entire canvas |
| **Zoom Canvas** | (`Cmd`+Wheel, `Ctrl`+Wheel) / Top Zoom Bar / `+` `-` Keys / Trackpad Pinch | Zoom in for cover details, zoom out for a bird's-eye overview |
| **Reset View** | Top Zoom Bar Center Button / Press `0` Key | Quickly recenter the camera viewport |
| **Online Playback** | Left-Click Video Card | Launch built-in streaming player directly |
| **View Details** | Right-Click Video Card | Open detailed popover (ID, cast, tags, and HD covers) |
| **Dynamic Preview** | Hover Mouse over Video Card | Automatically stream a muted short preview |
| **Search Videos** | Press `/` to search videos, press `Esc` to exit | Searches within the currently active site |
| **Search Tags** | Press `Cmd + k` or `Ctrl + k` in Navigation Drawer, press `Esc` to exit | Quickly filter and locate tags/categories |
| **Quick Download** | Click card top-left download button or add from details | Added tasks start paused; activate via bottom Download Manager |
| **Disconnect** | `Option+z` or `Alt+z` / Bottom-Left Disconnect Button | Instantly disconnect from site and display splash image, freeing your busy right hand |

---

## 📦 Which Package Should I Download?

AVDL provides both **Installer Packages** and **Portable Binaries** across all major platforms (6 download options in total):

> **Important**: `ffmpeg` must be installed on your system and available in your `PATH` environment variable. Download it from the official ffmpeg website or GitHub release page.

| OS | Architecture | File Format / Artifact | Description & Recommendation |
| :--- | :--- | :--- | :--- |
| **Windows** | x64 (64-bit) | `avdl_*_x64-setup.exe` | **Installer**: Standard installation wizard with desktop shortcut, recommended for most Windows users |
| | | `AVDL_*_windows_x64.exe` | **Portable**: Standalone executable, run directly without installation (ideal for USB drives) |
| **macOS** | Apple Silicon (arm64, M-series) | `avdl_*_aarch64.dmg` | **DMG Disk Image**: Double-click to mount and drag into your Applications folder |
| | | `AVDL_*_mac_arm64.zip` | **Portable Archive**: Extract the standalone executable directly |
| **Linux** | x64 (64-bit) | `avdl_*_amd64.AppImage` | **AppImage**: Universal across distros, grant executable permissions and run (Ubuntu / Fedora / Arch) |
| | | `AVDL_*_linux_x64.tar.gz` | **Portable Archive**: Extract the `avdl` standalone binary for advanced users & scripting |

> Note: Lowercase prefixes (e.g., `avdl_...`) represent standard installers; uppercase prefixes (e.g., `AVDL_...`) represent portable zero-install binaries.

---

## 🍎 macOS First-Launch Guide

Because this application is an open-source, unsigned release, macOS Gatekeeper may show a warning ("cannot be opened" or "damaged") on the first launch. Run the following command in **Terminal** to bypass the quarantine flag:

```bash
# For .app installed in the Applications folder:
xattr -cr /Applications/avdl.app

# For standalone portable executable:
xattr -cr ./avdl
```

After running this command, double-click to launch normally.

---

## 🛠️ Local Development & Build

To build AVDL from source:

1. **Prerequisites**:
   - Install [Bun](https://bun.sh/) (recommended) or Node.js (v18+)
   - Install [Rust](https://rustup.rs/) (1.75+)
   - Ensure `ffmpeg` is installed and added to `PATH`

2. **Install Dependencies**:

   ```bash
   bun install
   ```

3. **Start Development Server**:

   ```bash
   bun run tauri dev
   ```

4. **Production Build**:
   ```bash
   bun run tauri build
   ```

---

## 📜 Disclaimer

1. This project is intended solely for personal study, technical research, and media streaming exploration.
2. The software does not host, store, or upload any audio/video resources. All content is fetched directly from user-selected public web sources in real time.
3. Please use this software in compliance with local laws and regulations.
