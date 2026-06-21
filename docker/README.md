# Docker dev stack (x4mp-server)

Run the central WebSocket server in Docker for fast rebuild/redeploy. **SirNuke pipe server and x4mp-bridge stay on the Windows host** (X4 and Named Pipes are Windows-only).

Release assets: [bvbohnen/x4-projects releases](https://github.com/bvbohnen/x4-projects/releases)  
Pipe server asset used by default: `sn_x4_python_pipe_server_exe_v1.4.3.zip` (tag `v1.12`).

## Quick start (Windows)

From repo root:

```powershell
# Server only (Docker)
.\tools\dev-stack\start-server.ps1

# Full host stack: Docker server + pipe server + bridge
.\tools\dev-stack\start-host.ps1
```

Stop server:

```powershell
.\tools\dev-stack\stop-server.ps1
```

## Manual Compose

```powershell
docker compose up -d --build
curl http://127.0.0.1:7878/health
```

Bridge on host (server in Docker):

```powershell
$env:X4MP_SERVER_URL = "ws://127.0.0.1:7878/ws"
$env:X4MP_GAME_VERSION = "9.00"
cargo run -p x4mp-bridge
```

## Configuration

Copy `tools/dev-stack/env.example` to `tools/dev-stack/.env` to override ports and download URLs.

| Variable | Default | Purpose |
|----------|---------|---------|
| `X4MP_HOST_PORT` | `7878` | Host port published by Compose |
| `X4MP_SERVER_URL` | `ws://127.0.0.1:7878/ws` | Bridge WebSocket target |
| `X4MP_PIPE_SERVER_DOWNLOAD_URL` | GitHub release zip | SirNuke pipe server installer |

Session data persists in Docker volume `x4mp-data` (`/data` in container → event log, debug bundles).

## Spike / X4 workflow

1. `start-host.ps1` (or server + pipe server separately)
2. X4: Protected UI Mode **off**, SirNuke APIs + spike mod **on**
3. In-game: `SignalCues md.X4MP_Spike_Capture.SpikeCapture_Start`
4. Bridge still reads **stdin** in M0; pipe samples go through SirNuke pipe server until M1 wires pipe ↔ bridge

## Redeploy after code changes

```powershell
docker compose up -d --build
# or
.\tools\dev-stack\start-server.ps1 -Build
```
