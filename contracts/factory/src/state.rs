use cosmwasm_std::{Addr, Deps, Order};
use cw_storage_plus::{Bound, Item, Map};
use novaswap::{asset::AssetInfo, pairinfo::PairConfig};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Config {
    /// Address allowed to change contract parameters
    pub owner: Addr,
    /// CW20 token contract code identifier
    pub token_code_id: u64,
}

/// Structure for storing pair keys
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct TmpPairInfo {
    pub pair_key: Vec<u8>,
}

/// Saves a pair's key
pub const TMP_PAIR_INFO: Item<TmpPairInfo> = Item::new("tmp_pair_info");

/// Saves factory settings
pub const CONFIG: Item<Config> = Item::new("config");

/// Saves created pairs (from olders to latest)
pub const PAIRS: Map<&[u8], Addr> = Map::new("pair_info");

/// Calculate unique pair key from asset_infos
pub fn pair_key(asset_infos: &[AssetInfo; 2]) -> Vec<u8> {
    let mut asset_infos = asset_infos.to_vec();
    asset_infos.sort_by(|a, b| a.as_bytes().cmp(b.as_bytes()));

    [asset_infos[0].as_bytes(), asset_infos[1].as_bytes()].concat()
}

/// Saves pair type configurations
pub const PAIR_CONFIGS: Map<String, PairConfig> = Map::new("pair_configs");

/// The maximum limit for reading pairs from [`PAIRS`]
const MAX_LIMIT: u32 = 30;

/// The default limit for reading pairs from [`PAIRS`]
const DEFAULT_LIMIT: u32 = 10;

/// Reads pairs from the [`PAIRS`] vector according to the `start_after` and `limit` variables.
/// Otherwise, it returns the default number of pairs, starting from the oldest one.
pub fn read_pairs(
    deps: Deps,
    start_after: Option<[AssetInfo; 2]>,
    limit: Option<u32>,
) -> Vec<Addr> {
    let limit = limit.unwrap_or(DEFAULT_LIMIT).min(MAX_LIMIT) as usize;
    let start = calc_range_start(start_after).map(Bound::ExclusiveRaw);

    PAIRS
        .range(deps.storage, start, None, Order::Ascending)
        .take(limit)
        .map(|item| {
            let (_, pair_addr) = item.unwrap();
            pair_addr
        })
        .collect()
}

/// Calculates the key of a pair from which to start reading data.
fn calc_range_start(start_after: Option<[AssetInfo; 2]>) -> Option<Vec<u8>> {
    start_after.map(|asset_infos| {
        let mut asset_infos = asset_infos.to_vec();
        asset_infos.sort_by(|a, b| a.as_bytes().cmp(b.as_bytes()));

        let mut v = [asset_infos[0].as_bytes(), asset_infos[1].as_bytes()]
            .concat()
            .as_slice()
            .to_vec();
        v.push(1);
        v
    })
}
