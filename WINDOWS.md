# Building on Windows

`electrs-doge` uses conditional compilation so Linux/macOS builds are unchanged.

## Changes

- **`Cargo.toml`**: `hyperlocal` only on Unix (`[target.'cfg(unix)'.dependencies]`); `signal-hook` with `iterator` feature for Unix.
- **`src/rest.rs`**: Unix socket HTTP server gated with `#[cfg(unix)]`.
- **`src/signal.rs`**: Windows uses `signal_hook::flag` polling; no `SIGUSR1`.
- **`src/metrics.rs`**: Process stats stub on Windows (Prometheus gauges stay zero).

## Upstream (PsyProtocol / Blockstream Esplora)

Stay on **Blockstream `new-index` / Esplora**. Do **not** rebase onto [romanz/electrs](https://github.com/romanz/electrs) v0.11 (Feb 2026). That is current Bitcoin electrs, but a different product: compact Electrum-only index, **no Esplora HTTP**, blocks via **P2P** not `blk*.dat`. This stack needs Esplora (`dojak`, doge-sdk, command.dog proxy, dogexplorer).

The `doge` branch is Blockstream electrs with `rust-dogecoin` for AuxPoW + Dogecoin genesis. That part is real.

What was **not** retuned for Dogecoin L1 (fixed in this fork):

- JSON-RPC batches of **50,000** `getblockheader`s. Fine for 80-byte Bitcoin headers. Dogecoin headers include **AuxPoW**, so Core 1.14 drops the socket around `DAEMON_READ_TIMEOUT` (10 min) and electrs retries the same gulp forever. Index tip stays 0; Electrum/HTTP never bind. Now `JSONRPC_BATCH_SIZE = 64`. Log: `downloading headers START..=END (N/TIP)`.
- Default Core RPC **:8332** / `~/.bitcoin` (this fork defaults to **:22555** / `~/.dogecoin`; `dogenals launch` already passed the right flags).
- Pipeline depth **1** (Blockstream now uses **2**) and a **new rayon pool per blk file**.
- Whole-file `fs::read` plus **byte-scanning Core zero-padding** after the last real block (minutes per `blk*.dat`).

Bitcoin Core v28 xor-key on blk files: **skip**. Dogecoin 1.14 does not xor.

### dogex `blk_reader` (same machine, different job)

dogex is the metaprotocol indexer. Useful **I/O** ideas, not its protocol index:

| Steal | Leave in dogex |
|---|---|
| Sequential `blk*.dat` scan; **stop** on padding/bad magic | Opening Core `blocks/index` LevelDB while Core is running (`LOCK`) |
| Bounded batches (~32 MiB / 256 blocks) so RocksDB writes overlap the next read | Height-random prefetch via `blk-index` shadow |
| Windows `FILE_FLAG_SEQUENTIAL_SCAN` | `DOGEX_BLK_FULL_SCAN` / live-index repair |
| Progress that is visible in the log | Inscriptions / Ðunes / Treats in this process |

`--lightmode` stays the launch default (faster ingest; queries hit Core). `--jsonrpc-import` is slower; keep `FetchFrom::BlkFiles` after headers.

This fork: header batches of 64, pipeline depth 2, reused parse pool, sequential 32 MiB batches, RocksDB `increase_parallelism(num_cpus)`, compaction readahead 4 MiB, **256 MiB SST**, `max_open_files=1024`, **bloom + 256 MiB block cache**, 32 KiB table blocks on newly compacted files, `advise_random_on_open(true)` (history lookups are random). After txstore ingest, electrs **waits for L0 to drain** before history prevout lookups.

**History looks frozen (this PC):** txstore can sit at ~75 GB / ~900 SST files on the **F: WD Blue HDD**. Electrum/HTTP do not bind until history is done. A `dogenals tail electrs` stuck on `reading blk file 1/1353` with no new INFO lines means history `get()` is scanning every overlapping L0 file (~100 MB/s reads, ~0 writes, ~2 batches/night). Not a hung process — unusable I/O. Current binary compact-waits first (log: `compacting txstore before history (L0=…)` every 30s) then logs `history heights A..=B (N blocks, P prevouts, lookup …)` per batch.

## Runtime (this PC)

Electrs indexes **Dogecoin Core** (`dogecoin/` / `dogecoin-qt` RPC `:22555`). It cannot make progress if:

1. **Core is down** — log is only `failed to connect daemon … os error 10061`. Restart Qt yourself; never from this repo.
2. **`electrs.exe` is stale** — `target/release/electrs.exe` older than `src/daemon.rs` still uses Bitcoin-sized 50k `getblockheader` batches. AuxPoW replies from Core 1.14 never finish. Compile, then bounce **electrs only**.

Healthy ingest after Core warmup (`-28` Loading block index is fine). Electrs does **not** wait for Core IBD to finish — it indexes the validated tip and catches up.

```text
resume: 5276932 blocks already in txstore, 5276932 headers on disk — will not re-ingest those blocks
stored prefix through height 5276931 / 6342730 (83.2%)
header chain ready at height 5276931
downloading headers 5276932..=5277443 (5277444/6342731)
txstore checkpoint at height … — safe to interrupt
```

Not healthy: `downloading headers 0..=511 (512/TIP)` **after** a previous run already ingested millions of blocks (tip cookie missing — fixed: it rebuilds from the stored prefix). Also not healthy: `TRACE downloading 100000 block headers`, a full day of connection-refused, or only `waiting for bitcoind/dogecoind sync` with no header download (old binary). Logs say **dogecoind**, not bitcoind.

`failed to index N blocks from blk*.dat files` then process exit: the sequential `blk*.dat` scan finished but ~N tip headers were not in those files (Core still appending the last file, or the reader stopped on padding). **Old binaries panic.** Current `fetch.rs` pulls those leftovers from dogecoind RPC and keeps going. Compile + bounce **electrs only**.

Fewer than ~10k missing blocks (tip catch-up) go through dogecoind RPC. A full `blk*.dat` scan is only for bulk **history**. Do not expect `skipping block` spam — that was TRACE of every already-in-txstore hash.

### Resume after disk full / kill

Do **not** delete `F:\DogecoinData\electrs`. Free space, then bounce **electrs only**:

```text
electrs compile
dogenals kill electrs
dogenals launch electrs
```

- Header download writes `B{hash}` rows every 8k headers and flushes.
- Txstore/history flush every 4096 blocks.
- Tip cookie `t` is rewritten whenever the in-memory header chain is restored, not only at the end of a full update.
- AuxPoW is stripped in RAM (block hash is the 80-byte header); full headers stay in RocksDB.

Electrum protocol matches [romanz/electrs](https://github.com/romanz/electrs) 0.11.1 **method surface** (v1.4): `server.features`, `scripthash.unsubscribe`, version negotiation, JSON-RPC error objects, `transaction.get` verbose, estimatefee `-1` when Core has no estimate. Electrum `broadcast_package` is **not** implemented (Dogecoin Core 1.14 has no `submitpackage`). Esplora extras: `GET http://127.0.0.1:3003/electrum/features`.

### Esplora HTTP vs Blockstream [API.md](https://github.com/Blockstream/esplora/blob/master/API.md)

Local clone: `ref/esplora/` (frontend + `API.md`). electrs-doge stays **sync hyper 0.14** — do not rewrite the server to Blockstream’s tokio rest.rs.

Bitcoin / Dogecoin routes match that spec (`/tx`, `/address`, `/scripthash`, `/block`, `/blocks`, `/mempool`, `/fee-estimates`, `POST /tx`). Liquid-only `/asset*` stays behind `feature = "liquid"`.

Deliberate Dogecoin deltas:

| Endpoint | Blockstream | This fork |
|---|---|---|
| `POST /txs/package` | Core 28+ `submitpackage` | **501** JSON (`not_implemented`). Broadcast dependent txs in order via `POST /tx`. |
| `GET /block-template` | `--enable-mining-rest` → Core `getblocktemplate` (cached 15s, `Cache-Control: no-store`) | Same flag. Default **off**. Uses BIP 22 `{"mode":"template"}` — **no** Bitcoin `rules: ["segwit"]`. |

Do not enable `--enable-mining-rest` on the public tunnel unless you want miners hitting Core.

```powershell
electrs compile
electrs-doge kill
electrs-doge launch
dogenals tail electrs-doge
```

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

```text
electrs compile
electrs-doge kill
electrs-doge launch
```

Or: `cd electrs-doge && cargo build --release`. `electrs compile` parks a running `electrs.exe` so the linker can write (same as `dogenals compile`). Does not stop Core.
