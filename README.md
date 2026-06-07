# videdit

Bare-minimum video trimmer, compressor, and screen recorder for Windows.

- Lossless trim (cuts at nearest keyframe, no re-encode)
- Re-encode with compression (configurable CRF/preset)
- Multi-segment support with merge
- Screen recording with global hotkeys (F9 start, F10 stop) — captures display + system audio
- Keyboard-driven: Space, arrow keys, I/O for mark in/out

Built with Tauri v2, React, and FFmpeg.

## Install on Windows

Download the latest release from [GitHub Releases](https://github.com/asfarsadewa/videdit/releases) and run `videdit_X.Y.Z_x64-setup.exe`. The `.msi` asset is also published for managed installs.

The installer is currently unsigned, so Windows SmartScreen may show an extra confirmation prompt.

## Build

Requires [Node.js](https://nodejs.org/), [Rust](https://rustup.rs/), and FFmpeg/FFprobe binaries placed in `src-tauri/binaries/` with Tauri sidecar naming (`ffmpeg-x86_64-pc-windows-msvc.exe`). The helper script below downloads the Windows sidecars used by CI.

```
npm install
./scripts/prepare-windows-sidecars.ps1
npm run tauri build
```

Installers output to `src-tauri/target/release/bundle/`.
