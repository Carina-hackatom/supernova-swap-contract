use crate::mock_querier::mock_dependencies;
use cosmwasm_std::testing::MOCK_CONTRACT_ADDR;
use cosmwasm_std::testing::{mock_env, mock_info};
use cosmwasm_std::{attr, from_binary, to_binary, Addr, DepsMut, Env, MessageInfo, Reply, ReplyOn, Response, SubMsg, SubMsgResponse, SubMsgResult, WasmMsg, Uint128};
use novaswap::asset::AssetInfo;
use novaswap::factory::{ConfigResponse, ExecuteMsg, InstantiateMsg, QueryMsg};
use novaswap::pair::{InstantiateMsg as PairInstantiateMsg, StablePoolParams};
use novaswap::pairinfo::{PairConfig, PairInfo, PairType};
use novaswap::querier::query_factory_config;
use prost::Message;

use crate::contract::{execute, instantiate, query, reply};
use crate::error::ContractError;
use crate::state::CONFIG;

#[derive(Clone, PartialEq, Message)]
struct MsgInstantiateContractResponse {
    #[prost(string, tag = "1")]
    pub contract_address: ::prost::alloc::string::String,
    #[prost(bytes, tag = "2")]
    pub data: ::prost::alloc::vec::Vec<u8>,
}

fn init_contract(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    owner: String,
) -> Result<Response, ContractError> {
    let msg = InstantiateMsg {
        pair_configs: vec![PairConfig {
            code_id: 1u64,
            pair_type: PairType::Stable {},
            is_disabled: false,
        }],
        token_code_id: 1u64,
        owner,
    };

    instantiate(deps, env, info, msg)
}

#[test]
fn proper_initialization() {
    let mut deps = mock_dependencies();
    let owner = "owner0000".to_string();
    let msg = InstantiateMsg {
        pair_configs: vec![PairConfig {
            code_id: 1u64,
            pair_type: PairType::Stable {},
            is_disabled: false,
        }],
        token_code_id: 1u64,
        owner: owner.clone(),
    };

    let env = mock_env();
    let info = mock_info("addr0000", &[]);

    instantiate(deps.as_mut(), env.clone(), info, msg.clone()).unwrap();

    let query_res = query(deps.as_ref(), env, QueryMsg::Config {}).unwrap();
    let config: ConfigResponse = from_binary(&query_res).unwrap();

    assert_eq!(1u64, config.token_code_id);
    assert_eq!(msg.pair_configs, config.pair_configs);
    assert_eq!(Addr::unchecked(owner), config.owner);
}

#[test]
fn duplicated_initialization() {
    let mut deps = mock_dependencies();
    let owner = "owner0000".to_string();
    let msg = InstantiateMsg {
        pair_configs: vec![
            PairConfig {
                code_id: 1u64,
                pair_type: PairType::Stable {},
                is_disabled: false,
            },
            PairConfig {
                code_id: 1u64,
                pair_type: PairType::Stable {},
                is_disabled: false,
            },
        ],
        token_code_id: 1u64,
        owner,
    };

    let env = mock_env();
    let info = mock_info("addr0000", &[]);

    let err = instantiate(deps.as_mut(), env, info, msg).unwrap_err();
    assert_eq!(err, ContractError::PairConfigDuplicate {});
}

#[test]
fn update_config() {
    let mut deps = mock_dependencies();
    let owner = "owner0000".to_string();
    let env = mock_env();
    let info = mock_info("addr0000", &[]);

    // init
    init_contract(deps.as_mut(), env.clone(), info, owner).unwrap();

    // err: the user is not an owner.
    let info = mock_info("addr0000", &[]);
    let msg = ExecuteMsg::UpdateOwner {
        new_owner: "addr0000".to_string(),
    };
    let res = execute(deps.as_mut(), env.clone(), info, msg.clone()).unwrap_err();
    assert_eq!(res, ContractError::Unauthorized {});

    // check current owner is owner0000
    let result = query(deps.as_ref(), env.clone(), QueryMsg::Config {}).unwrap();
    let config: ConfigResponse = from_binary(&result).unwrap();
    assert_eq!(config.owner, "owner0000");

    // it works. He is an owner.
    let info = mock_info("owner0000", &[]);
    execute(deps.as_mut(), env.clone(), info, msg).unwrap();

    // now new owner applied
    let result = query(deps.as_ref(), env, QueryMsg::Config {}).unwrap();
    let config: ConfigResponse = from_binary(&result).unwrap();
    assert_eq!(config.owner, "addr0000");
}

#[test]
fn update_owner() {
    let mut deps = mock_dependencies();
    let owner = "owner0000".to_string();
    let env = mock_env();
    let info = mock_info("addr0000", &[]);

    // init
    init_contract(deps.as_mut(), env.clone(), info, owner).unwrap();

    // err: the user is not an owner.
    let info = mock_info("addr0000", &[]);
    let msg = ExecuteMsg::UpdateConfig {
        token_code_id: Some(2u64),
    };
    let res = execute(deps.as_mut(), env.clone(), info, msg.clone()).unwrap_err();
    assert_eq!(res, ContractError::Unauthorized {});

    // it works. He is an owner.
    let info = mock_info("owner0000", &[]);
    execute(deps.as_mut(), env.clone(), info, msg).unwrap();

    let result = query(deps.as_ref(), env, QueryMsg::Config {}).unwrap();
    let config: ConfigResponse = from_binary(&result).unwrap();
    assert_eq!(config.token_code_id, 2u64);
}

#[test]
fn unauthorized_update_pair_config() {
    let mut deps = mock_dependencies();
    let owner = "owner0000".to_string();
    let env = mock_env();
    let info = mock_info("addr0000", &[]);

    init_contract(deps.as_mut(), env.clone(), info.clone(), owner).unwrap();
    let msg = ExecuteMsg::UpdatePairConfig {
        config: PairConfig {
            code_id: 5,
            pair_type: PairType::Stable {},
            is_disabled: false,
        },
    };

    let err = execute(deps.as_mut(), env, info, msg).unwrap_err();
    assert_eq!(err, ContractError::Unauthorized {});
}

#[test]
fn update_pair_config() {
    let mut deps = mock_dependencies();
    let owner = "owner0000".to_string();
    let env = mock_env();
    let info = mock_info("owner0000", &[]);

    init_contract(deps.as_mut(), env.clone(), info.clone(), owner).unwrap();

    // check initial contract has proper pair types
    let result = query(deps.as_ref(), env.clone(), QueryMsg::Config {}).unwrap();
    let config: ConfigResponse = from_binary(&result).unwrap();
    assert_eq!(
        config.pair_configs,
        vec![PairConfig {
            code_id: 1u64,
            pair_type: PairType::Stable {},
            is_disabled: false
        }]
    );

    let new_pair_config = PairConfig {
        code_id: 5u64,
        pair_type: PairType::Stable {},
        is_disabled: false,
    };

    let msg = ExecuteMsg::UpdatePairConfig {
        config: new_pair_config.clone(),
    };

    execute(deps.as_mut(), env.clone(), info, msg).unwrap();

    // check updated pair configs.
    let result = query(deps.as_ref(), env, QueryMsg::Config {}).unwrap();
    let config: ConfigResponse = from_binary(&result).unwrap();
    assert_eq!(config.pair_configs, vec![new_pair_config]);
}

#[test]
fn add_pair_config() {
    let mut deps = mock_dependencies();
    let owner = "owner0000".to_string();
    let env = mock_env();
    let info = mock_info("owner0000", &[]);

    init_contract(deps.as_mut(), env.clone(), info.clone(), owner).unwrap();

    let current_pair_config = PairConfig {
        code_id: 1u64,
        pair_type: PairType::Stable {},
        is_disabled: false,
    };

    let new_pair_config = PairConfig {
        code_id: 6,
        pair_type: PairType::Xyk {},
        is_disabled: false,
    };

    let msg = ExecuteMsg::UpdatePairConfig {
        config: new_pair_config.clone(),
    };

    execute(deps.as_mut(), env.clone(), info, msg).unwrap();

    let result = query(deps.as_ref(), env, QueryMsg::Config {}).unwrap();
    let config: ConfigResponse = from_binary(&result).unwrap();

    // check new pair type has been added.
    assert_eq!(
        config.pair_configs,
        vec![current_pair_config, new_pair_config]
    );
}

#[test]
fn create_pair() {
    let mut deps = mock_dependencies();
    let owner = "owner0000".to_string();
    let env = mock_env();
    let info = mock_info("owner0000", &[]);

    init_contract(deps.as_mut(), env.clone(), info.clone(), owner).unwrap();

    let asset_infos = [
        AssetInfo::Token {
            contract_addr: Addr::unchecked("token0000"),
        },
        AssetInfo::Token {
            contract_addr: Addr::unchecked("token0001"),
        },
    ];

    let config = CONFIG.load(&deps.storage).unwrap();
    let res = execute(
        deps.as_mut(),
        env,
        info,
        ExecuteMsg::CreatePair {
            pair_type: PairType::Stable {},
            asset_infos: asset_infos.clone(),
            init_params: None,
        },
    )
    .unwrap();

    assert_eq!(
        res.attributes,
        vec![
            attr("action", "create_pair"),
            attr("pair", "token0000-token0001")
        ]
    );

    assert_eq!(
        res.messages,
        vec![SubMsg {
            msg: WasmMsg::Instantiate {
                msg: to_binary(&PairInstantiateMsg {
                    factory_addr: String::from(MOCK_CONTRACT_ADDR),
                    asset_infos,
                    token_code_id: 1u64,
                    init_params: None
                })
                .unwrap(),
                code_id: 1u64,
                funds: vec![],
                admin: Some(config.owner.to_string()),
                label: String::from("Novaswap pair"),
            }
            .into(),
            id: 1,
            gas_limit: None,
            reply_on: ReplyOn::Success
        }]
    );
}

#[test]
fn cannot_create_pair() {
    let mut deps = mock_dependencies();
    let owner = "owner0000".to_string();
    let env = mock_env();
    let info = mock_info("owner0000", &[]);

    let msg = InstantiateMsg {
        pair_configs: vec![PairConfig {
            code_id: 1u64,
            pair_type: PairType::Stable {},
            is_disabled: false,
        }],
        token_code_id: 1u64,
        owner,
    };

    instantiate(deps.as_mut(), env.clone(), info.clone(), msg).unwrap();

    let asset_infos = [
        AssetInfo::Token {
            contract_addr: Addr::unchecked("token0000"),
        },
        AssetInfo::Token {
            contract_addr: Addr::unchecked("token0001"),
        },
    ];

    let err = execute(
        deps.as_mut(),
        env,
        info,
        ExecuteMsg::CreatePair {
            pair_type: PairType::Xyk {},
            asset_infos,
            init_params: None,
        },
    )
    .unwrap_err();

    assert_eq!(err, ContractError::PairConfigNotFound {});
}

#[test]
fn register() {
    let mut deps = mock_dependencies();
    let owner = "owner0000".to_string();
    let env = mock_env();
    let info = mock_info("owner0000", &[]);

    let msg = InstantiateMsg {
        pair_configs: vec![PairConfig {
            code_id: 1u64,
            pair_type: PairType::Stable {},
            is_disabled: false,
        }],
        token_code_id: 1u64,
        owner,
    };

    instantiate(deps.as_mut(), env, info, msg).unwrap();

    // 1. create pair
    let asset_infos = [
        AssetInfo::Token {
            contract_addr: Addr::unchecked("asset0000"),
        },
        AssetInfo::Token {
            contract_addr: Addr::unchecked("asset0001"),
        },
    ];

    let msg = ExecuteMsg::CreatePair {
        pair_type: PairType::Stable {},
        asset_infos: asset_infos.clone(),
        init_params: Some(to_binary(&StablePoolParams { amp: 50 }).unwrap()),
    };
    let env = mock_env();
    let info = mock_info("addr0000", &[]);
    let _res = execute(deps.as_mut(), env.clone(), info, msg).unwrap();

    // 2. apply mock querier
    let pair0_addr = "pair0000".to_string();
    let pair0_info = PairInfo {
        asset_infos: asset_infos.clone(),
        contract_addr: Addr::unchecked("pair0000"),
        liquidity_token: Addr::unchecked("liquidity0000"),
        pair_type: PairType::Stable {},
    };

    let deployed_pairs = vec![(&pair0_addr, &pair0_info)];
    deps.querier.with_novaswap_pairs(&deployed_pairs);

    // reply for creating pair contract.
    let data = MsgInstantiateContractResponse {
        contract_address: String::from("pair0000"),
        data: vec![],
    };

    let mut encoded_instantiate_reply = Vec::<u8>::with_capacity(data.encoded_len());
    data.encode(&mut encoded_instantiate_reply).unwrap();

    let reply_msg = Reply {
        id: 1,
        result: SubMsgResult::Ok(SubMsgResponse {
            events: vec![],
            data: Some(encoded_instantiate_reply.into()),
        }),
    };

    let _res = reply(deps.as_mut(), mock_env(), reply_msg.clone()).unwrap();

    // 4. query created pair infos.
    let query_res = query(
        deps.as_ref(),
        env,
        QueryMsg::Pair {
            asset_infos: asset_infos.clone(),
        },
    )
    .unwrap();

    let pair_res: PairInfo = from_binary(&query_res).unwrap();
    assert_eq!(
        pair_res,
        PairInfo {
            liquidity_token: Addr::unchecked("liquidity0000"),
            contract_addr: Addr::unchecked("pair0000"),
            asset_infos,
            pair_type: PairType::Stable {},
        }
    );

    let err = reply(deps.as_mut(), mock_env(), reply_msg).unwrap_err();
    assert_eq!(err, ContractError::PairWasRegistered {});
}

#[test]
fn test_calculate_optimal_price_ratio() {
    let reserve_b = Uint128::new(100);
    let reserve_a = Uint128::new(150);
    let amount_a = Uint128::new(53);

    let amount_b = reserve_b * amount_a / reserve_a;
    let optimal_amount_b  = amount_b + Uint128::new(1);

    assert_eq!(optimal_amount_b, Uint128::new(36));
}