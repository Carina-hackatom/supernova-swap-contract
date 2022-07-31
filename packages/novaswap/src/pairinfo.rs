use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::asset::{Asset, AssetInfo};
use cosmwasm_std::{Addr, QuerierWrapper, StdResult};
use std::fmt::{Display, Formatter, Result};

/// This enum describes available pair types.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PairType {
    /// XYK pair type
    Xyk {},
    /// Stable pair type
    Stable {},
}

/// This struct describes pair info of created pools;
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct PairInfo {
    pub asset_infos: [AssetInfo; 2],
    pub contract_addr: Addr,
    pub liquidity_token: Addr,
    pub pair_type: PairType,
}

impl PairInfo {
    pub fn query_pools(
        &self,
        querier: &QuerierWrapper,
        contract_addr: Addr,
    ) -> StdResult<[Asset; 2]> {
        Ok([
            Asset {
                amount: self.asset_infos[0].query_pool(querier, contract_addr.clone())?,
                info: self.asset_infos[0].clone(),
            },
            Asset {
                amount: self.asset_infos[1].query_pool(querier, contract_addr)?,
                info: self.asset_infos[1].clone(),
            },
        ])
    }
}

// Return a name of each pool type
impl Display for PairType {
    fn fmt(&self, fmt: &mut Formatter) -> Result {
        match self {
            PairType::Xyk {} => fmt.write_str("xyk"),
            PairType::Stable {} => fmt.write_str("stable"),
        }
    }
}

// This structure stores a pair type's configuration
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct PairConfig {
    /// ID of contract which is allowed to create pairs of this type
    pub code_id: u64,
    /// The pair type (provided in a [`PairType`])
    pub pair_type: PairType,
    /// Whether a pair type is disabled or not. If it is disabled, new pairs cannot be
    /// created, but existing ones can still read the pair configuration
    pub is_disabled: bool,
}
