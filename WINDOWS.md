# Building on Windows

`electrs-doge` uses conditional compilation so Linux/macOS builds are unchanged.

## Changes

- **`Cargo.toml`**: `hyperlocal` only on Unix (`[target.'cfg(unix)'.dependencies]`); `signal-hook` with `iterator` feature for Unix.
- **`src/rest.rs`**: Unix socket HTTP server gated with `#[cfg(unix)]`.
- **`src/signal.rs`**: Windows uses `signal_hook::flag` polling; no `SIGUSR1`.
- **`src/metrics.rs`**: Process stats stub on Windows (Prometheus gauges stay zero).

## Stack (dogenals launch)

`dogenals launch` starts electrs-doge with the rest of the eco:

- Electrum TCP `127.0.0.1:50001` — dogexplorer `/address/` pages (`DOGEXP_ADDRESS_API=electrum`)
- Esplora HTTP `127.0.0.1:3003` — **not** `:3000` (that is command.dog/api). Public: `https://electrs.command.dog`
- Index DB: `%DOGECOIN_DATA_DIR%\electrs` (default `F:\DogecoinData\electrs`) — read Core blocks/RPC only; never stop Core
- Logs: `F:\DogecoinData\dogenals\logs\electrs-doge.log`
- Skip: `DOGENALS_SKIP_ELECTRS=1`

## Limitations on Windows

- No `--http-socket-file` (TCP `http://127.0.0.1:3003` only).
- No `SIGUSR1` re-index trigger via `blocknotify`.
- Prometheus process metrics (CPU/RSS/fds) are zero.

## Build

```powershell
cd electrs-doge
cargo build --release
.\scripts\bin\electrs-doge-launch.ps1
```
