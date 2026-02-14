# videdit

Bare-minimum video trimmer, compressor, and screen recorder for Windows.

- Lossless trim (cuts at nearest keyframe, no re-encode)
- Re-encode with compression (configurable CRF/preset)
- Multi-segment support with merge
- Screen recording with global hotkeys (F9 start, F10 stop) — captures display + system audio
- Keyboard-driven: Space, arrow keys, I/O for mark in/out

Built with Tauri v2, React, and FFmpeg.

## Build

Requires [Node.js](https://nodejs.org/), [Rust](https://rustup.rs/), and FFmpeg/FFprobe binaries placed in `src-tauri/binaries/` with Tauri sidecar naming (`ffmpeg-x86_64-pc-windows-msvc.exe`).

```
npm install
npm run tauri build
```

Installers output to `src-tauri/target/release/bundle/`.
