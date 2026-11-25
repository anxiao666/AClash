# AClash

Lightweight Windows tray client for [mihomo](https://github.com/MetaCubeX/mihomo).

## What it does

- Start/stop the mihomo core
- Import airport Clash/Mihomo YAML (URL or file)
- Import company WireGuard config separately
- Fixed routing: company → WG, China/LAN → DIRECT, others → PROXY
- Manual PROXY node selection
- TUN toggle (requires Administrator)
- Connections and logs

## Develop

Requirements:

- Node.js 20+
- Rust stable
- Visual Studio Build Tools (MSVC)
- WebView2 runtime

```bash
npm install
npm run download-core
npm run tauri dev
```

`download-core` places `mihomo.exe` at `src-tauri/resources/mihomo.exe`.

On Windows, use an MSVC developer environment (Visual Studio Build Tools) before `tauri dev/build`.

## Build

```bash
npm run tauri build
```

If release builds OOM on low-memory machines, use debug:

```bash
npm run tauri build -- --debug
```

## Data directory

`%APPDATA%\AClash\`

Legacy data from `%APPDATA%\mihomo-lite\` is migrated automatically on first launch.
