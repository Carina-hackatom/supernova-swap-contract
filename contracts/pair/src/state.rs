use cosmwasm_std::{Addr, Uint128};
use cw_storage_plus::Item;
use novaswap::pairinfo::PairInfo;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Config {
    pub pair_info: PairInfo,
    pub factory_addr: Addr,

    // used for calculatring TAWP.
    pub block_time_last: u64,
    pub price0_cumulative_last: Uint128,
    pub price1_cumulative_last: Uint128,

    // used for stableswap pair algorithm.
    pub init_amp: u64,
    pub init_amp_time: u64,
    pub next_amp: u64,
    pub next_amp_time: u64,
}

pub const CONFIG: Item<Config> = Item::new("config");
