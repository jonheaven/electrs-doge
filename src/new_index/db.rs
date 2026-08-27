use rocksdb;

use std::path::Path;
use std::time::Duration;

use crate::config::Config;
use crate::errors::*;
use crate::util::{bincode, Bytes};

static DB_VERSION: u32 = 1;

#[derive(Debug, Eq, PartialEq)]
pub struct DBRow {
    pub key: Vec<u8>,
    pub value: Vec<u8>,
}

pub struct ScanIterator<'a> {
    prefix: Vec<u8>,
    iter: rocksdb::DBIterator<'a>,
    done: bool,
}

impl<'a> Iterator for ScanIterator<'a> {
    type Item = DBRow;

    fn next(&mut self) -> Option<DBRow> {
        if self.done {
            return None;
        }
        let (key, value) = self.iter.next()?.expect("valid iterator");
        if !key.starts_with(&self.prefix) {
            self.done = true;
            return None;
        }
        Some(DBRow {
            key: key.to_vec(),
            value: value.to_vec(),
        })
    }
}

pub struct ReverseScanIterator<'a> {
    prefix: Vec<u8>,
    iter: rocksdb::DBRawIterator<'a>,
    done: bool,
}

impl<'a> Iterator for ReverseScanIterator<'a> {
    type Item = DBRow;

    fn next(&mut self) -> Option<DBRow> {
        if self.done || !self.iter.valid() {
            return None;
        }

        let key = self.iter.key().unwrap();
        if !key.starts_with(&self.prefix) {
            self.done = true;
            return None;
        }

        let row = DBRow {
            key: key.into(),
            value: self.iter.value().unwrap().into(),
        };

        self.iter.prev();

        Some(row)
    }
}

#[derive(Debug)]
pub struct DB {
    db: rocksdb::DB,
}

#[derive(Copy, Clone, Debug)]
pub enum DBFlush {
    Disable,
    Enable,
}

impl DB {
    pub fn open(path: &Path, config: &Config) -> DB {
        debug!("opening DB at {:?}", path);
        let mut db_opts = rocksdb::Options::default();
        db_opts.create_if_missing(true);
        // History prevout lookups are random. 256 fds + 935 overlapping L0 SSTs on a
        // spinning disk made each get() re-open files and scan every L0 SST.
        db_opts.set_max_open_files(1024);
        db_opts.set_compaction_style(rocksdb::DBCompactionStyle::Level);
        db_opts.set_compression_type(rocksdb::DBCompressionType::Snappy);
        db_opts.set_target_file_size_base(256 << 20); // 256 MiB SST (was 1 GiB)
        db_opts.set_write_buffer_size(256 << 20);
        db_opts.set_disable_auto_compactions(true); // for initial bulk load
        // Random gets (history O{txid} lookups), not sequential blk ingest.
        db_opts.set_advise_random_on_open(true);
        debug!("configured rocksdb options at {:?}", path);
        db_opts.set_compaction_readahead_size(4 << 20);
        let parallelism = num_cpus::get().max(2) as i32;
        db_opts.increase_parallelism(parallelism);
        let mut block_opts = rocksdb::BlockBasedOptions::default();
        // 1 MiB blocks + no bloom: each miss on HDD reads 1 MiB from every L0 file.
        // New SSTs after compaction use 32 KiB + bloom; existing files rewrite on compact.
        block_opts.set_block_size(32 << 10);
        block_opts.set_bloom_filter(10.0, false);
        let cache = rocksdb::Cache::new_lru_cache(256 * 1024 * 1024);
        block_opts.set_block_cache(&cache);
        block_opts.set_cache_index_and_filter_blocks(true);
        block_opts.set_pin_l0_filter_and_index_blocks_in_cache(true);
        db_opts.set_block_based_table_factory(&block_opts);
        debug!("finalized rocksdb options at {:?}", path);

        debug!("running rocksdb open at {:?}", path);
        let db = DB {
            db: rocksdb::DB::open(&db_opts, path).expect("failed to open RocksDB"),
        };
        debug!("verifying compatibility at {:?}", path);
        db.verify_compatibility(config);
        debug!("opened DB at {:?}", path);
        db
    }

    pub fn full_compaction(&self) {
        // TODO: make sure this doesn't fail silently
        debug!("starting full compaction on {:?}", self.db);
        self.db.compact_range(None::<&[u8]>, None::<&[u8]>);
        debug!("finished full compaction on {:?}", self.db);
    }

    pub fn enable_auto_compaction(&self) {
        let opts = [("disable_auto_compactions", "false")];
        self.db.set_options(&opts).unwrap();
    }

    pub fn files_at_level(&self, level: u32) -> u64 {
        let prop = format!("rocksdb.num-files-at-level{}", level);
        self.db.property_int_value(prop).ok().flatten().unwrap_or(0)
    }

    pub fn level_file_summary(&self) -> String {
        (0..7)
            .map(|lvl| format!("L{}={}", lvl, self.files_at_level(lvl)))
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// History prevout `get()` searches every overlapping L0 SST. On an HDD with
    /// ~900 L0 files that is ~100 MB/s of random reads and years of wall time.
    /// Drain L0 (compaction gets the disk *without* lookups fighting it) first.
    pub fn wait_until_l0_drained(&self) {
        self.enable_auto_compaction();
        const TARGET_L0: u64 = 16;
        const STALL_TICKS: u32 = 40; // 20 min with no L0 drop
        let levels_total: u64 = (0..7).map(|lvl| self.files_at_level(lvl)).sum();
        if levels_total == 0 {
            warn!(
                "txstore level file counts are all 0 — RocksDB properties unavailable? cannot wait on L0"
            );
            return;
        }
        let mut last = u64::MAX;
        let mut stalled = 0u32;
        loop {
            let l0 = self.files_at_level(0);
            if l0 <= TARGET_L0 {
                info!(
                    "txstore ready for history lookups ({}, L0={})",
                    self.level_file_summary(),
                    l0
                );
                return;
            }
            if l0 >= last {
                stalled += 1;
            } else {
                stalled = 0;
            }
            info!(
                "compacting txstore before history ({} ; L0 {} → ≤{}) — do not expect Electrum/HTTP until this finishes",
                self.level_file_summary(),
                l0,
                TARGET_L0
            );
            if stalled >= STALL_TICKS {
                warn!(
                    "txstore L0 stuck at {} for 20m (disk space or compaction stall). Starting history anyway.",
                    l0
                );
                return;
            }
            last = l0;
            std::thread::sleep(Duration::from_secs(30));
        }
    }

    pub fn raw_iterator(&self) -> rocksdb::DBRawIterator<'_> {
        self.db.raw_iterator()
    }

    pub fn iter_scan(&self, prefix: &[u8]) -> ScanIterator<'_> {
        ScanIterator {
            prefix: prefix.to_vec(),
            iter: self.db.prefix_iterator(prefix),
            done: false,
        }
    }

    pub fn iter_scan_from(&self, prefix: &[u8], start_at: &[u8]) -> ScanIterator<'_> {
        let iter = self.db.iterator(rocksdb::IteratorMode::From(
            start_at,
            rocksdb::Direction::Forward,
        ));
        ScanIterator {
            prefix: prefix.to_vec(),
            iter,
            done: false,
        }
    }

    pub fn iter_scan_reverse(&self, prefix: &[u8], prefix_max: &[u8]) -> ReverseScanIterator<'_> {
        let mut iter = self.db.raw_iterator();
        iter.seek_for_prev(prefix_max);

        ReverseScanIterator {
            prefix: prefix.to_vec(),
            iter,
            done: false,
        }
    }

    pub fn write(&self, rows: Vec<DBRow>, flush: DBFlush) {
        self.try_write(rows, flush)
            .unwrap_or_else(|e| panic!("{}", e));
    }

    pub fn try_write(&self, mut rows: Vec<DBRow>, flush: DBFlush) -> Result<()> {
        debug!(
            "writing {} rows to {:?}, flush={:?}",
            rows.len(),
            self.db,
            flush
        );
        rows.sort_unstable_by(|a, b| a.key.cmp(&b.key));
        let mut batch = rocksdb::WriteBatch::default();
        for row in rows {
            #[cfg(not(feature = "oldcpu"))]
            batch.put(&row.key, &row.value);
            #[cfg(feature = "oldcpu")]
            batch.put(&row.key, &row.value).unwrap();
        }
        let do_flush = match flush {
            DBFlush::Enable => true,
            DBFlush::Disable => false,
        };
        let mut opts = rocksdb::WriteOptions::new();
        opts.set_sync(do_flush);
        opts.disable_wal(!do_flush);
        self.db.write_opt(batch, &opts).chain_err(|| {
            "RocksDB write failed. If the disk is full, free space and relaunch electrs — already-flushed blocks resume. Do not delete the electrs db."
        })?;
        Ok(())
    }

    pub fn flush(&self) {
        self.try_flush()
            .unwrap_or_else(|e| panic!("{}", e));
    }

    pub fn try_flush(&self) -> Result<()> {
        self.db.flush().chain_err(|| {
            "RocksDB flush failed. If the disk is full, free space and relaunch electrs — already-flushed blocks resume. Do not delete the electrs db."
        })?;
        Ok(())
    }

    pub fn put(&self, key: &[u8], value: &[u8]) {
        self.db.put(key, value).unwrap();
    }

    pub fn put_sync(&self, key: &[u8], value: &[u8]) {
        let mut opts = rocksdb::WriteOptions::new();
        opts.set_sync(true);
        self.db.put_opt(key, value, &opts).unwrap();
    }

    pub fn get(&self, key: &[u8]) -> Option<Bytes> {
        self.db.get(key).unwrap().map(|v| v.to_vec())
    }

    fn verify_compatibility(&self, config: &Config) {
        let mut compatibility_bytes = bincode::serialize_little(&DB_VERSION).unwrap();

        if config.light_mode {
            // append a byte to indicate light_mode is enabled.
            // we're not letting bincode serialize this so that the compatiblity bytes won't change
            // (and require a reindex) when light_mode is disabled. this should be chagned the next
            // time we bump DB_VERSION and require a re-index anyway.
            compatibility_bytes.push(1);
        }

        match self.get(b"V") {
            None => self.put(b"V", &compatibility_bytes),
            Some(ref x) if x != &compatibility_bytes => {
                panic!("Incompatible database found. Please reindex.")
            }
            Some(_) => (),
        }
    }
}
