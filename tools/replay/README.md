# x4mp-replay

Replay `data/sessions/<session_id>/event.log` from a given sequence against a running WebSocket server — for post-mortems without launching X4.

## CLI

```bash
cargo run -p x4mp-replay -- --session <uuid> --from-seq N --server ws://127.0.0.1:7878/ws
```

Optional flags:

- `--data-dir data` — root containing `sessions/<id>/event.log`
- `--display-name ReplayBot` — handshake display name
- `--join-code ABCD-1234` — dev join code (M0 default)

## Behavior

1. Reads NDJSON lines from the session event log
2. Filters out `handshake` / `handshake.ack` and events with `seq < from-seq`
3. Connects to the server, performs a fresh handshake
4. Sends each remaining event; server assigns new seq values on echo

## Example

After running `tools/dev-echo.ps1`, replay the dev session log:

```bash
cargo run -p x4mp-replay -- \
  --session 11111111-1111-1111-1111-111111111111 \
  --from-seq 1
```
