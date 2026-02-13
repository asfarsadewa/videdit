# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build & Development Commands

```bash
npm run tauri dev          # Launch app in development mode (starts Vite + Rust)
npm run tauri build        # Production build (creates NSIS installer)
npx tsc --noEmit           # TypeScript type check (frontend only)
npx vite build             # Frontend build only
cd src-tauri && cargo check # Rust type check only
cd src-tauri && cargo build # Rust full build only
```

## Architecture

Tauri v2 desktop app: Rust backend spawns FFmpeg/FFprobe as child processes, React frontend renders video via the HTML5 `<video>` element using Tauri's asset protocol.

### Frontend → Backend Communication

- **Tauri commands** (`invoke`): `get_video_info(path)` and `export_video(inputPath, segments, outputPath, merge)` defined in `src-tauri/src/lib.rs`
- **Tauri events** (`listen`/`emit`): `export-progress` event emitted from Rust during export, listened to in `ExportPanel.tsx`
- **Asset protocol**: Video files are served to the `<video>` element via `convertFileSrc(path)` which produces `https://asset.localhost/` URLs

### Backend (src-tauri/src/)

- `lib.rs` — Tauri app setup, plugin registration, command handler registration
- `ffmpeg.rs` — All FFmpeg/FFprobe interaction. Spawns processes directly via `std::process::Command` (not Tauri's shell plugin). Uses `hide_console_window()` on Windows to suppress console popups. Export flow: cut segments to temp files → optionally concat merge → emit progress events.

### Frontend (src/)

- `App.tsx` — All app state lives here (video info, segments, current time). Manages segment add/update/delete with overlap validation.
- `components/VideoPlayer.tsx` — `<video>` wrapper with keyboard shortcuts (Space, arrows, I/O for mark in/out)
- `components/Timeline.tsx` — Draggable timeline with segment handles (start/end/body drag)
- `components/ExportPanel.tsx` — Export UI with progress bar, listens to `export-progress` events
- `components/SegmentList.tsx` — Segment list display with delete and seek-to-time

### FFmpeg Sidecars

Binaries live at `src-tauri/binaries/ffmpeg-x86_64-pc-windows-msvc.exe` and `ffprobe-x86_64-pc-windows-msvc.exe`. They are gitignored. The Tauri sidecar naming convention requires the platform triple suffix — config references them without suffix as `binaries/ffmpeg`. At runtime, paths resolve via `app.path().resource_dir()`.

## Key Conventions

- **Tauri v2 capabilities**: Permission identifiers use `core:` prefix for built-in features (e.g., `core:event:default` not `event:default`). Defined in `src-tauri/capabilities/default.json`.
- **Tailwind CSS v4**: Uses `@import "tailwindcss"` in CSS (not `@tailwind` directives). Plugin configured via `@tailwindcss/vite`.
- **Lossless only**: All video cuts use FFmpeg `-c copy`. Cuts snap to nearest keyframe — this is inherent and by design.
- **All state in App.tsx**: No state management library. Segment state is `Array<{ id, start, end }>`, sorted by start time, with overlap prevention.
