mod server;
pub use server::RPC;

#[cfg(feature = "electrum-discovery")]
mod client;
#[cfg(feature = "electrum-discovery")]
mod discovery;
#[cfg(feature = "electrum-discovery")]
pub use {client::Client, discovery::DiscoveryManager};

use std::cmp::Ordering;
use std::collections::HashMap;
use std::str::FromStr;

use serde::{de, Deserialize, Deserializer, Serialize};

use crate::chain::{genesis_hash, BlockHash};
use crate::config::Config;
use crate::errors::*;
use crate::util::BlockId;
use serde_json::Value;

pub fn get_electrum_height(blockid: Option<BlockId>, has_unconfirmed_parents: bool) -> isize {
    match (blockid, has_unconfirmed_parents) {
        (Some(blockid), _) => blockid.height as isize,
        (None, false) => 0,
        (None, true) => -1,
    }
}

pub type Port = u16;
pub type Hostname = String;

pub type ServerHosts = HashMap<Hostname, ServerPorts>;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ServerFeatures {
    pub hosts: ServerHosts,
    pub genesis_hash: BlockHash,
    pub server_version: String,
    pub protocol_min: ProtocolVersion,
    pub protocol_max: ProtocolVersion,
    pub pruning: Option<usize>,
    pub hash_function: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ServerPorts {
    tcp_port: Option<Port>,
    ssl_port: Option<Port>,
}

#[derive(Eq, PartialEq, Debug, Clone, Default)]
pub struct ProtocolVersion {
    major: usize,
    minor: usize,
}

impl ProtocolVersion {
    pub const fn new(major: usize, minor: usize) -> Self {
        Self { major, minor }
    }
}

impl Ord for ProtocolVersion {
    fn cmp(&self, other: &Self) -> Ordering {
        self.major
            .cmp(&other.major)
            .then_with(|| self.minor.cmp(&other.minor))
    }
}

impl PartialOrd for ProtocolVersion {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl FromStr for ProtocolVersion {
    type Err = crate::errors::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut iter = s.split('.');
        Ok(Self {
            major: iter
                .next()
                .chain_err(|| "missing major")?
                .parse()
                .chain_err(|| "invalid major")?,
            minor: iter
                .next()
                .chain_err(|| "missing minor")?
                .parse()
                .chain_err(|| "invalid minor")?,
        })
    }
}

impl std::fmt::Display for ProtocolVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}", self.major, self.minor)
    }
}

/// Electrum protocol version compare (romanz/electrs 0.9.14+).
pub fn protocol_in_range(ours: &str, min: &str, max: &str) -> crate::errors::Result<()> {
    fn parse(version: &str) -> crate::errors::Result<Vec<usize>> {
        version
            .split('.')
            .map(|part| {
                part.parse::<usize>()
                    .chain_err(|| format!("invalid protocol version {}", version))
            })
            .collect()
    }
    let version = parse(ours)?;
    let min_v = parse(min)?;
    let max_v = parse(max)?;
    if version < min_v {
        bail!("version {} < {}", ours, min);
    }
    if version > max_v {
        bail!("version {} > {}", ours, max);
    }
    Ok(())
}

pub const PROTOCOL_VERSION: ProtocolVersion = ProtocolVersion::new(1, 4);

pub fn server_id() -> String {
    format!("electrs-doge/{}", env!("CARGO_PKG_VERSION"))
}

/// Electrum `server.features` body (also served on Esplora `GET /electrum/features`).
pub fn local_server_features(config: &Config) -> Value {
    let addr = config.electrum_rpc_addr;
    json!({
        "genesis_hash": genesis_hash(config.network_type),
        "hosts": {
            addr.ip().to_string(): { "tcp_port": addr.port() }
        },
        "protocol_max": PROTOCOL_VERSION,
        "protocol_min": PROTOCOL_VERSION,
        "pruning": Value::Null,
        "server_version": server_id(),
        "hash_function": "sha256",
        "electrum_rpc": format!("tcp://{}", addr),
    })
}

impl Serialize for ProtocolVersion {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.collect_str(&self)
    }
}

impl<'de> Deserialize<'de> for ProtocolVersion {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        FromStr::from_str(&s).map_err(de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::protocol_in_range;

    #[test]
    fn protocol_range_matches_romanz_0_9_14() {
        assert!(protocol_in_range("1.4", "1.4", "1.4").is_ok());
        assert!(protocol_in_range("1.4", "1.4", "1.5").is_ok());
        assert!(protocol_in_range("1.4", "1.3", "1.4").is_ok());
        assert!(protocol_in_range("1.4", "1.5", "1.5").is_err());
        assert!(protocol_in_range("1.4", "1.3", "1.3").is_err());
    }
}
