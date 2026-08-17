# Building on Windows

`electrs-doge` uses conditional compilation so Linux/macOS builds are unchanged.

## Changes

- **`Cargo.toml`**: `hyperlocal` only on Unix (`[target.'cfg(unix)'.dependencies]`); `signal-hook` with `iterator` feature for Unix.
- **`src/rest.rs`**: Unix socket HTTP server gated with `#[cfg(unix)]`.
- **`src/signal.rs`**: Windows uses `signal_hook::flag` polling; no `SIGUSR1`.
- **`src/metrics.rs`**: Process stats stub on Windows (Prometheus gauges stay zero).

## Upstream (PsyProtocol / Blockstream)

The `doge` branch is Blockstream electrs with `rust-dogecoin` swapped in for AuxPoW + Dogecoin genesis. That part is real.

What was **not** retuned for Dogecoin L1:

- JSON-RPC batches of **50,000** `getblockheader`s. Fine for 80-byte Bitcoin headers. Dogecoin headers include **AuxPoW**, so Core 1.14 drops the socket around `DAEMON_READ_TIMEOUT` (10 min) and electrs retries the same gulp forever. Index tip stays 0; Electrum/HTTP never bind.
- Default Core RPC **:8332** / `~/.bitcoin` (this fork now defaults to **:22555** / `~/.dogecoin`; `dogenals launch` already passed the right flags).

Header download then blk*.dat index is the same design as Bitcoin electrs. It was incomplete for **mainnet Dogecoin volume + AuxPoW**, not missing a protocol.

This fork: `JSONRPC_BATCH_SIZE = 64`. Log lines `downloading headers START..=END (N/TIP)`.

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
