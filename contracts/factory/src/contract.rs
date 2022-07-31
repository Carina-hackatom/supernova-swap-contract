use std::collections::HashSet;

use cosmwasm_std::{
    attr, entry_point, to_binary, Binary, Deps, DepsMut, Env, MessageInfo, Order, Reply, ReplyOn,
    Response, StdResult, SubMsg, WasmMsg,
};
use cw2::set_contract_version;
use novaswap::asset::AssetInfo;
use novaswap::factory::{
    ConfigResponse, ExecuteMsg, InstantiateMsg, MigrateMsg, PairsResponse, QueryMsg,
};
use novaswap::pair::InstantiateMsg as PairInstantiateMsg;
use novaswap::pairinfo::{PairConfig, PairInfo, PairType};

use crate::error::ContractError;
use crate::querier::query_pair_info;
use crate::state::{
    pair_key, read_pairs, Config, TmpPairInfo, CONFIG, PAIRS, PAIR_CONFIGS, TMP_PAIR_INFO,
};

/// Contract name that is used for migration.
const CONTRACT_NAME: &str = "novaswap-factory";

/// Contract version that is used for migration.
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

/// A `reply` call code ID used in a sub-message.
const INSTANTIATE_PAIR_REPLY_ID: u64 = 1;

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    let config = Config {
        owner: deps.api.addr_validate(&msg.owner)?,
        token_code_id: msg.token_code_id,
    };

    let config_set: HashSet<String> = msg
        .pair_configs
        .iter()
        .map(|pc| pc.pair_type.to_string())
        .collect();

    if config_set.len() != msg.pair_configs.len() {
        return Err(ContractError::PairConfigDuplicate {});
    }

    for pc in msg.pair_configs.iter() {
        PAIR_CONFIGS.save(deps.storage, pc.clone().pair_type.to_string(), pc)?;
    }

    CONFIG.save(deps.storage, &config)?;

    Ok(Response::new())
}

/// Data structure used to update general contract parameters.
pub struct UpdateConfig {
    /// This is the CW20 token contract code identifier
    token_code_id: Option<u64>,
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::UpdateConfig { token_code_id } => {
            execute_update_config(deps, info, UpdateConfig { token_code_id })
        }
        ExecuteMsg::UpdatePairConfig { config } => execute_update_pair_config(deps, info, config),
        ExecuteMsg::CreatePair {
            pair_type,
            asset_infos,
            init_params,
        } => execute_create_pair(deps, env, pair_type, asset_infos, init_params),
        ExecuteMsg::UpdateOwner { new_owner } => execute_update_owner(deps, info, new_owner),
    }
}

fn execute_update_config(
    deps: DepsMut,
    info: MessageInfo,
    params: UpdateConfig,
) -> Result<Response, ContractError> {
    let mut config = CONFIG.load(deps.storage)?;

    if info.sender != config.owner {
        return Err(ContractError::Unauthorized {});
    }

    if let Some(token_code_id) = params.token_code_id {
        config.token_code_id = token_code_id;
    }

    CONFIG.save(deps.storage, &config)?;

    Ok(Response::new().add_attribute("action", "update_config"))
}

fn execute_update_owner(
    deps: DepsMut,
    info: MessageInfo,
    new_owner: String,
) -> Result<Response, ContractError> {
    let mut config = CONFIG.load(deps.storage)?;

    if info.sender != config.owner {
        return Err(ContractError::Unauthorized {});
    }

    config.owner = deps.api.addr_validate(new_owner.as_str())?;
    CONFIG.save(deps.storage, &config)?;
    Ok(Response::new())
}

fn execute_update_pair_config(
    deps: DepsMut,
    info: MessageInfo,
    pair_config: PairConfig,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;

    if info.sender != config.owner {
        return Err(ContractError::Unauthorized {});
    }

    PAIR_CONFIGS.save(
        deps.storage,
        pair_config.pair_type.to_string(),
        &pair_config,
    )?;

    Ok(Response::new().add_attribute("action", "update_pair_config"))
}

fn execute_create_pair(
    deps: DepsMut,
    env: Env,
    pair_type: PairType,
    asset_infos: [AssetInfo; 2],
    init_params: Option<Binary>,
) -> Result<Response, ContractError> {
    asset_infos[0].check(deps.api)?;
    asset_infos[1].check(deps.api)?;

    if asset_infos[0] == asset_infos[1] {
        return Err(ContractError::DoublingAssets {});
    }

    let config = CONFIG.load(deps.storage)?;

    if PAIRS
        .may_load(deps.storage, &pair_key(&asset_infos))?
        .is_some()
    {
        return Err(ContractError::PairWasCreated {});
    }

    // Get pair type from config
    let pair_config = PAIR_CONFIGS
        .load(deps.storage, pair_type.to_string())
        .map_err(|_| ContractError::PairConfigNotFound {})?;

    // Check if pair config is disabled
    if pair_config.is_disabled {
        return Err(ContractError::PairConfigDisabled {});
    }

    let pair_key = pair_key(&asset_infos);
    TMP_PAIR_INFO.save(deps.storage, &TmpPairInfo { pair_key })?;

    let sub_msg: Vec<SubMsg> = vec![SubMsg {
        id: INSTANTIATE_PAIR_REPLY_ID,
        msg: WasmMsg::Instantiate {
            admin: Some(config.owner.to_string()),
            code_id: pair_config.code_id,
            msg: to_binary(&PairInstantiateMsg {
                asset_infos: asset_infos.clone(),
                token_code_id: config.token_code_id,
                factory_addr: env.contract.address.to_string(),
                init_params,
            })?,
            funds: vec![],
            label: "Novaswap pair".to_string(),
        }
        .into(),
        gas_limit: None,
        reply_on: ReplyOn::Success,
    }];

    Ok(Response::new()
        .add_submessages(sub_msg)
        .add_attributes(vec![
            attr("action", "create_pair"),
            attr("pair", format!("{}-{}", asset_infos[0], asset_infos[1])),
        ]))
}

/// The entry point to the contract for processing replies from submessages.
#[cfg_attr(not(feature = "library"), entry_point)]
pub fn reply(deps: DepsMut, _env: Env, msg: Reply) -> Result<Response, ContractError> {
    let tmp = TMP_PAIR_INFO.load(deps.storage)?;
    if PAIRS.may_load(deps.storage, &tmp.pair_key)?.is_some() {
        return Err(ContractError::PairWasRegistered {});
    }

    let res = cw_utils::parse_reply_instantiate_data(msg).unwrap();
    let pair_contract = deps.api.addr_validate(res.contract_address.as_str())?;

    PAIRS.save(deps.storage, &tmp.pair_key, &pair_contract)?;

    Ok(Response::new().add_attributes(vec![
        attr("action", "register"),
        attr("pair_contract_addr", pair_contract),
    ]))
}

/// Expose all queries available in the contract.
#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Config {} => to_binary(&query_config(deps)?),
        QueryMsg::Pair { asset_infos } => to_binary(&query_pair(deps, asset_infos)?),
        QueryMsg::Pairs { start_after, limit } => {
            to_binary(&query_pairs(deps, start_after, limit)?)
        }
    }
}

pub fn query_config(deps: Deps) -> StdResult<ConfigResponse> {
    let config = CONFIG.load(deps.storage)?;
    let resp = ConfigResponse {
        owner: config.owner,
        token_code_id: config.token_code_id,
        pair_configs: PAIR_CONFIGS
            .range(deps.storage, None, None, Order::Ascending)
            .map(|item| {
                let (_, cfg) = item.unwrap();
                cfg
            })
            .collect(),
    };

    Ok(resp)
}

pub fn query_pair(deps: Deps, asset_infos: [AssetInfo; 2]) -> StdResult<PairInfo> {
    let pair_addr = PAIRS.load(deps.storage, &pair_key(&asset_infos))?;
    query_pair_info(deps, &pair_addr)
}

pub fn query_pairs(
    deps: Deps,
    start_after: Option<[AssetInfo; 2]>,
    limit: Option<u32>,
) -> StdResult<PairsResponse> {
    let pairs: Vec<PairInfo> = read_pairs(deps, start_after, limit)
        .iter()
        .map(|pair_addr| query_pair_info(deps, pair_addr).unwrap())
        .collect();

    Ok(PairsResponse { pairs })
}

// Used for contract migration.
#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(_deps: DepsMut, _env: Env, _msg: MigrateMsg) -> Result<Response, ContractError> {
    Ok(Response::default())
}
