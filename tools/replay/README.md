# x4mp-replay (M0.5)

M0.5 deliverable: replay `event.log` from a given sequence against a running server or mock consumer.

## Expected CLI (M0.5)

```bash
cargo run -p x4mp-replay -- --session <id> --from-seq N
```

## Purpose

Post-mortem debugging without launching X4. Reads `data/sessions/<session_id>/event.log` and re-submits envelopes from `--from-seq` onward.

M0 establishes the append-only `event.log` format; the replay binary lands in M0.5.
