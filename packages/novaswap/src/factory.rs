use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{
    asset::AssetInfo,
    pairinfo::{PairConfig, PairInfo, PairType},
};
use cosmwasm_std::{Addr, Binary};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InstantiateMsg {
    // pair_configs : stableswap인지 xyk인지 정보가 저장되어 있다.
    pub pair_configs: Vec<PairConfig>,

    // token_code_id : LP토큰 생성할 때 어떤 코드로 생성할 건지 저장
    pub token_code_id: u64,

    // owner : config 설정 관리하는 관리자 지갑 주소
    pub owner: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExecuteMsg {
    UpdateConfig {
        token_code_id: Option<u64>,
    },
    UpdatePairConfig {
        config: PairConfig,
    },
    CreatePair {
        pair_type: PairType,
        asset_infos: [AssetInfo; 2],
        init_params: Option<Binary>,
    },
    UpdateOwner {
        new_owner: String,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    Config {},
    Pair {
        asset_infos: [AssetInfo; 2],
    },
    Pairs {
        start_after: Option<[AssetInfo; 2]>,
        limit: Option<u32>,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct ConfigResponse {
    pub owner: Addr,
    pub pair_configs: Vec<PairConfig>,
    pub token_code_id: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct PairsResponse {
    pub pairs: Vec<PairInfo>,
}

// This structure stores the parameters used in a migration message.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct MigrateMsg {
    pub params: Binary,
}
