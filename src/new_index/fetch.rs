use rayon::prelude::*;

#[cfg(feature = "liquid")]
use crate::elements::ebcompact::*;
#[cfg(not(feature = "liquid"))]
use bitcoin::consensus::encode::deserialize;
#[cfg(feature = "liquid")]
use elements::encode::deserialize;

use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::{BufReader, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::mpsc::Receiver;
use std::thread;

use crate::chain::{Block, BlockHash};
use crate::daemon::Daemon;
use crate::errors::*;
use crate::util::{spawn_thread, HeaderEntry, SyncChannel};

/// Overlap disk read, CPU parse, and RocksDB write. Current Blockstream `new-index` uses 2;
/// this fork used to use 1 and stalled the disk reader behind every RocksDB batch.
const BLK_PIPELINE: usize = 2;

/// dogex `BlkReader` / sidelane prefetch: sequential read, then a bounded CPU batch so
/// the indexer can write while the next slice of the same `blk*.dat` is still coming in.
/// Whole-file `fs::read` (~128 MiB) plus byte-scanning Core zero-padding was the slow path.
const BLK_BATCH_BYTES: usize = 32 * 1024 * 1024;
const BLK_BATCH_BLOCKS: usize = 256;
const DOGE_MAX_BLOCK_BYTES: u32 = 8_000_000;

#[cfg(windows)]
const FILE_FLAG_SEQUENTIAL_SCAN: u32 = 0x0800_0000;

#[derive(Clone, Copy, Debug)]
pub enum FetchFrom {
    Bitcoind,
    BlkFiles,
}

pub fn start_fetcher(
    from: FetchFrom,
    daemon: &Daemon,
    new_headers: Vec<HeaderEntry>,
) -> Result<Fetcher<Vec<BlockEntry>>> {
    let fetcher = match from {
        FetchFrom::Bitcoind => bitcoind_fetcher,
        FetchFrom::BlkFiles => blkfiles_fetcher,
    };
    fetcher(daemon, new_headers)
}

pub struct BlockEntry {
    pub block: Block,
    pub entry: HeaderEntry,
    pub size: u32,
}

type SizedBlock = (Block, u32);

pub struct Fetcher<T> {
    receiver: Receiver<T>,
    thread: thread::JoinHandle<()>,
}

impl<T> Fetcher<T> {
    fn from(receiver: Receiver<T>, thread: thread::JoinHandle<()>) -> Self {
        Fetcher { receiver, thread }
    }

    pub fn map<F>(self, mut func: F)
    where
        F: FnMut(T) -> (),
    {
        for item in self.receiver {
            func(item);
        }
        self.thread.join().expect("fetcher thread panicked")
    }
}

fn bitcoind_fetcher(
    daemon: &Daemon,
    new_headers: Vec<HeaderEntry>,
) -> Result<Fetcher<Vec<BlockEntry>>> {
    if let Some(tip) = new_headers.last() {
        debug!("{:?} ({} left to index)", tip, new_headers.len());
    };
    let daemon = daemon.reconnect()?;
    let chan = SyncChannel::new(BLK_PIPELINE);
    let sender = chan.sender();
    Ok(Fetcher::from(
        chan.into_receiver(),
        spawn_thread("bitcoind_fetcher", move || {
            for entries in new_headers.chunks(100) {
                let blockhashes: Vec<BlockHash> = entries.iter().map(|e| *e.hash()).collect();
                let blocks = daemon
                    .getblocks(&blockhashes)
                    .expect("failed to get blocks from bitcoind");
                assert_eq!(blocks.len(), entries.len());
                let block_entries: Vec<BlockEntry> = blocks
                    .into_iter()
                    .zip(entries)
                    .map(|(block, entry)| BlockEntry {
                        entry: entry.clone(), // TODO: remove this clone()
                        size: block.total_size() as u32,
                        block,
                    })
                    .collect();
                assert_eq!(block_entries.len(), entries.len());
                sender
                    .send(block_entries)
                    .expect("failed to send fetched blocks");
            }
        }),
    ))
}

fn blkfiles_fetcher(
    daemon: &Daemon,
    new_headers: Vec<HeaderEntry>,
) -> Result<Fetcher<Vec<BlockEntry>>> {
    let magic = daemon.magic();
    let blk_files = daemon.list_blk_files()?;

    let chan = SyncChannel::new(BLK_PIPELINE);
    let sender = chan.sender();
    let mut entry_map: HashMap<BlockHash, HeaderEntry> =
        new_headers.into_iter().map(|h| (*h.hash(), h)).collect();

    let parser = blkfiles_parser(blkfiles_reader(blk_files, magic));
    Ok(Fetcher::from(
        chan.into_receiver(),
        spawn_thread("blkfiles_fetcher", move || {
            parser.map(|sizedblocks| {
                let block_entries: Vec<BlockEntry> = sizedblocks
                    .into_iter()
                    .filter_map(|(block, size)| {
                        let blockhash = block.block_hash();
                        entry_map
                            .remove(&blockhash)
                            .map(|entry| BlockEntry { block, entry, size })
                            .or_else(|| {
                                trace!("skipping block {}", blockhash);
                                None
                            })
                    })
                    .collect();
                if block_entries.is_empty() {
                    return;
                }
                trace!("fetched {} blocks", block_entries.len());
                sender
                    .send(block_entries)
                    .expect("failed to send blocks entries from blk*.dat files");
            });
            if !entry_map.is_empty() {
                panic!(
                    "failed to index {} blocks from blk*.dat files",
                    entry_map.len()
                )
            }
        }),
    ))
}

fn blkfiles_reader(blk_files: Vec<PathBuf>, magic: u32) -> Fetcher<Vec<(Vec<u8>, u32)>> {
    let chan = SyncChannel::new(BLK_PIPELINE);
    let sender = chan.sender();

    Fetcher::from(
        chan.into_receiver(),
        spawn_thread("blkfiles_reader", move || {
            let n = blk_files.len();
            for (i, path) in blk_files.into_iter().enumerate() {
                info!("reading blk file {}/{} {:?}", i + 1, n, path);
                read_blk_file_batches(&path, magic, |batch| {
                    sender.send(batch).unwrap_or_else(|_| {
                        panic!("failed to send {:?} contents", path)
                    });
                });
            }
        }),
    )
}

fn blkfiles_parser(blobs: Fetcher<Vec<(Vec<u8>, u32)>>) -> Fetcher<Vec<SizedBlock>> {
    let chan = SyncChannel::new(BLK_PIPELINE);
    let sender = chan.sender();

    Fetcher::from(
        chan.into_receiver(),
        spawn_thread("blkfiles_parser", move || {
            let pool = rayon::ThreadPoolBuilder::new()
                .num_threads(0)
                .thread_name(|i| format!("parse-blocks-{}", i))
                .build()
                .unwrap();
            blobs.map(|batch| {
                if batch.is_empty() {
                    return;
                }
                trace!("parsing {} raw blocks", batch.len());
                let blocks = parse_block_blobs(&pool, batch);
                sender
                    .send(blocks)
                    .expect("failed to send blocks from blk*.dat file");
            });
        }),
    )
}

fn open_blk_sequential(path: &Path) -> std::io::Result<File> {
    let mut opts = OpenOptions::new();
    opts.read(true);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        opts.custom_flags(FILE_FLAG_SEQUENTIAL_SCAN);
    }
    opts.open(path)
}

/// Sequential Core `blk*.dat` scan (same idea as dogex `BlkReader::scan_file`).
/// Stop on magic mismatch / oversize — Core pads files with zeros; byte-walking
/// that padding (old Blockstream parser) is a multi-minute stall per file.
fn read_next_raw_block(
    r: &mut BufReader<File>,
    magic: u32,
) -> Option<(Vec<u8>, u32)> {
    loop {
        let pos = r.stream_position().ok()?;
        let mut mag = [0u8; 4];
        if r.read_exact(&mut mag).is_err() {
            return None;
        }
        let value = u32::from_le_bytes(mag);
        if value != magic {
            return None;
        }
        let mut szb = [0u8; 4];
        if r.read_exact(&mut szb).is_err() {
            return None;
        }
        let block_size = u32::from_le_bytes(szb);
        if block_size == 0 || block_size > DOGE_MAX_BLOCK_BYTES {
            return None;
        }
        // Core WriteBlockToDisk ftell failure: magic+size written, body missing.
        // First payload u32 then equals magic — skip this truncated record.
        let mut peek = [0u8; 4];
        if r.read_exact(&mut peek).is_err() {
            return None;
        }
        if u32::from_le_bytes(peek) == magic {
            let _ = r.seek(SeekFrom::Start(pos + 8));
            continue;
        }
        let mut blob = vec![0u8; block_size as usize];
        blob[..4].copy_from_slice(&peek);
        if r.read_exact(&mut blob[4..]).is_err() {
            return None;
        }
        return Some((blob, block_size));
    }
}

fn read_blk_file_batches(path: &Path, magic: u32, mut send: impl FnMut(Vec<(Vec<u8>, u32)>)) {
    let file = open_blk_sequential(path)
        .unwrap_or_else(|e| panic!("failed to read {:?}: {:?}", path, e));
    let mut reader = BufReader::with_capacity(8 << 20, file);
    let mut batch: Vec<(Vec<u8>, u32)> = Vec::new();
    let mut batch_bytes = 0usize;
    while let Some((blob, size)) = read_next_raw_block(&mut reader, magic) {
        batch_bytes += blob.len();
        batch.push((blob, size));
        if batch_bytes >= BLK_BATCH_BYTES || batch.len() >= BLK_BATCH_BLOCKS {
            send(std::mem::take(&mut batch));
            batch_bytes = 0;
        }
    }
    if !batch.is_empty() {
        send(batch);
    }
}

fn parse_block_blobs(pool: &rayon::ThreadPool, batch: Vec<(Vec<u8>, u32)>) -> Vec<SizedBlock> {
    pool.install(|| {
        batch
            .into_par_iter()
            .map(|(slice, size)| (deserialize(&slice).expect("failed to parse Block"), size))
            .collect()
    })
}
