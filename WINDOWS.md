# Building on Windows

`electrs-doge` uses conditional compilation so Linux/macOS builds are unchanged.

## Changes

- **`Cargo.toml`**: `hyperlocal` only on Unix (`[target.'cfg(unix)'.dependencies]`); `signal-hook` with `iterator` feature for Unix.
- **`src/rest.rs`**: Unix socket HTTP server gated with `#[cfg(unix)]`.
- **`src/signal.rs`**: Windows uses `signal_hook::flag` polling; no `SIGUSR1`.
- **`src/metrics.rs`**: Process stats stub on Windows (Prometheus gauges stay zero).

## Limitations on Windows

- No `--http-socket-file` (TCP `http://127.0.0.1:3000` only).
- No `SIGUSR1` re-index trigger via `blocknotify`.
- Prometheus process metrics (CPU/RSS/fds) are zero.

## Build

```powershell
cd electrs-doge
cargo build --release
.\scripts\bin\electrs-doge-launch.ps1
```
