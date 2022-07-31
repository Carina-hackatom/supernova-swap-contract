use std::ops::Mul;

use crate::{
    contract::{execute, instantiate, query_pair_info, query_pool, query_share, query_simulation},
    error::ContractError,
    math::AMP_PRECISION,
    mock_querier::mock_dependencies,
    state::Config,
    test_helpers::{
        mock_env_with_block_time, store_liquidity_token, swap_token, CASES, TEST_SWAP_DECIMALS,
        TOKEN_DECIMALS,
    },
    utils::{accumulate_prices, assert_max_spread},
};
use cosmwasm_std::{
    attr,
    testing::{mock_env, mock_info, MOCK_CONTRACT_ADDR},
    to_binary, Addr, BankMsg, Coin, CosmosMsg, Decimal, ReplyOn, StdError, SubMsg, Uint128,
    WasmMsg,
};
use cw20::{Cw20ExecuteMsg, Cw20ReceiveMsg, MinterResponse};
use novaswap::token::InstantiateMsg as TokenInstantiateMsg;
use novaswap::{
    asset::{Asset, AssetInfo},
    pair::{
        Cw20HookMsg, ExecuteMsg, InstantiateMsg, PoolResponse, SimulationResponse,
        StablePoolParams, TWAP_PRECISION,
    },
    pairinfo::{PairInfo, PairType},
    U256,
};
use crate::math::MINIMUM_AMP;

#[test]
fn proper_initialization() {
    let mut deps = mock_dependencies(&[]);

    deps.querier.with_token_balances(&[(
        &String::from("asset0000"),
        &[(&String::from(MOCK_CONTRACT_ADDR), &Uint128::new(123u128))],
    )]);

    let msg = InstantiateMsg {
        asset_infos: [
            AssetInfo::Token {
                contract_addr: Addr::unchecked("asset0000"),
            },
            AssetInfo::NativeToken {
                denom: "uusd".to_string(),
            },
        ],
        token_code_id: 1u64,
        factory_addr: String::from("factory0000"),
        init_params: Some(to_binary(&StablePoolParams { amp: 50u64 }).unwrap()),
    };

    let env = mock_env();
    let info = mock_info("owner0000", &[]);
    let res = instantiate(deps.as_mut(), env, info, msg).unwrap();

    assert_eq!(
        res.messages,
        vec![SubMsg {
            msg: WasmMsg::Instantiate {
                code_id: 1u64,
                msg: to_binary(&TokenInstantiateMsg {
                    name: "SNT-UUSD-LP".to_string(),
                    symbol: "uLP".to_string(),
                    decimals: 6,
                    initial_balances: vec![],
                    mint: Some(MinterResponse {
                        minter: String::from(MOCK_CONTRACT_ADDR),
                        cap: None,
                    }),
                })
                .unwrap(),
                funds: vec![],
                admin: None,
                label: String::from("Novaswap LP token"),
            }
            .into(),
            id: 1,
            gas_limit: None,
            reply_on: ReplyOn::Success,
        }]
    );

    // Store liquidity token
    store_liquidity_token(deps.as_mut(), 1, "liquidity0000".to_string());

    // It worked, let's query the state
    let pair_info: PairInfo = query_pair_info(deps.as_ref()).unwrap();
    assert_eq!(Addr::unchecked("liquidity0000"), pair_info.liquidity_token);
    assert_eq!(
        pair_info.asset_infos,
        [
            AssetInfo::Token {
                contract_addr: Addr::unchecked("asset0000")
            },
            AssetInfo::NativeToken {
                denom: "uusd".to_string(),
            }
        ]
    );
}

#[test]
fn initialization_should_fail() {
    let mut deps = mock_dependencies(&[]);
    let env = mock_env();
    let info = mock_info("owner0000", &[]);

    // Check the pair cannot be created with same assets.
    let msg = InstantiateMsg {
        asset_infos: [
            AssetInfo::Token {
                contract_addr: Addr::unchecked("asset0000"),
            },
            AssetInfo::Token {
                contract_addr: Addr::unchecked("asset0000"),
            },
        ],
        token_code_id: 1u64,
        factory_addr: String::from("factory0000"),
        init_params: None,
    };

    let err = instantiate(deps.as_mut(), env.clone(), info.clone(), msg).unwrap_err();
    assert_eq!(err, ContractError::DoublingAssets {});

    // Check init params should be filled
    let msg = InstantiateMsg {
        asset_infos: [
            AssetInfo::Token {
                contract_addr: Addr::unchecked("asset0000"),
            },
            AssetInfo::Token {
                contract_addr: Addr::unchecked("asset0001"),
            },
        ],
        token_code_id: 1u64,
        factory_addr: String::from("factory0000"),
        init_params: None,
    };

    let err = instantiate(deps.as_mut(), env.clone(), info.clone(), msg).unwrap_err();
    assert_eq!(err, ContractError::InitParamsNotFound {});

    // Check Incorrect amp
    let msg = InstantiateMsg {
        asset_infos: [
            AssetInfo::Token {
                contract_addr: Addr::unchecked("asset0000"),
            },
            AssetInfo::Token {
                contract_addr: Addr::unchecked("asset0001"),
            },
        ],
        token_code_id: 1u64,
        factory_addr: String::from("factory0000"),
        init_params: Some(to_binary(&StablePoolParams { amp: 0u64 }).unwrap()),
    };

    let err = instantiate(deps.as_mut(), env, info, msg).unwrap_err();
    assert_eq!(err, ContractError::IncorrectAmp {});
}

/// 1. 정상적으로 유동성 풀에 공급했는지 확인한다.
/// 2. 유동성 풀을 1:2 비율로 공급하고 받는 LP토큰의 개수를 확인한다.
/// 3. 실제보다 적은 코인을 제출해본다.
#[test]
fn test_provide_liquidity() {
    let mut deps = mock_dependencies(&[Coin {
        denom: "uusd".to_string(),
        amount: Uint128::new(200u128).mul(Uint128::new(TOKEN_DECIMALS)),
    }]);

    deps.querier.with_token_balances(&[
        (
            &String::from("asset0000"),
            &[(&String::from(MOCK_CONTRACT_ADDR), &Uint128::new(0))],
        ),
        (
            &String::from("liquidity0000"),
            &[(&String::from(MOCK_CONTRACT_ADDR), &Uint128::new(0))],
        ),
    ]);

    let msg = InstantiateMsg {
        asset_infos: [
            AssetInfo::NativeToken {
                denom: "uusd".to_string(),
            },
            AssetInfo::Token {
                contract_addr: Addr::unchecked("asset0000"),
            },
        ],
        token_code_id: 10u64,
        factory_addr: String::from("factory"),
        init_params: Some(to_binary(&StablePoolParams { amp: 100 }).unwrap()),
    };

    let env = mock_env();
    let info = mock_info("addr0000", &[]);

    let _res = instantiate(deps.as_mut(), env, info, msg).unwrap();
    store_liquidity_token(deps.as_mut(), 1, "liquidity0000".to_string());

    // Successfully provide liquidity for the existing pool
    let msg = ExecuteMsg::ProvideLiquidity {
        assets: [
            Asset {
                info: AssetInfo::Token {
                    contract_addr: Addr::unchecked("asset0000"),
                },
                amount: Uint128::from(100u128).mul(Uint128::from(TOKEN_DECIMALS)),
            },
            Asset {
                info: AssetInfo::NativeToken {
                    denom: "uusd".to_string(),
                },
                amount: Uint128::from(100u128).mul(Uint128::from(TOKEN_DECIMALS)),
            },
        ],
        receiver: None,
    };

    let env = mock_env();
    let info = mock_info(
        "addr0000",
        &[Coin {
            denom: "uusd".to_string(),
            amount: Uint128::from(100u128).mul(Uint128::from(TOKEN_DECIMALS)),
        }],
    );
    let res = execute(deps.as_mut(), env, info, msg).unwrap();
    let transfer_from_msg = res.messages.get(0).expect("no message");
    let mint_msg = res.messages.get(1).expect("no message");

    // should execute `TransferFrom` on cw20 contract.
    assert_eq!(
        transfer_from_msg,
        &SubMsg {
            msg: WasmMsg::Execute {
                contract_addr: String::from("asset0000"),
                msg: to_binary(&Cw20ExecuteMsg::TransferFrom {
                    owner: String::from("addr0000"),
                    recipient: String::from(MOCK_CONTRACT_ADDR),
                    amount: Uint128::from(100u128).mul(Uint128::from(TOKEN_DECIMALS)),
                })
                .unwrap(),
                funds: vec![],
            }
            .into(),
            id: 0,
            gas_limit: None,
            reply_on: ReplyOn::Never,
        },
    );

    // should mint lp token to the sender.
    assert_eq!(
        mint_msg,
        &SubMsg {
            msg: WasmMsg::Execute {
                contract_addr: String::from("liquidity0000"),
                msg: to_binary(&Cw20ExecuteMsg::Mint {
                    recipient: String::from("addr0000"),
                    amount: Uint128::from(100u128).mul(Uint128::from(TOKEN_DECIMALS)),
                })
                .unwrap(),
                funds: vec![],
            }
            .into(),
            id: 0,
            gas_limit: None,
            reply_on: ReplyOn::Never,
        }
    );

    // Provide more liquidity using a 1:2 ratio
    // **user deposit must be pre-applied**
    deps.querier.with_balance(&[(
        &String::from(MOCK_CONTRACT_ADDR),
        &[Coin {
            denom: "uusd".to_string(),
            amount: Uint128::new(400u128).mul(Uint128::from(TOKEN_DECIMALS)),
        }],
    )]);

    deps.querier.with_token_balances(&[
        (
            &String::from("liquidity0000"),
            &[(
                &String::from(MOCK_CONTRACT_ADDR),
                &Uint128::new(100u128).mul(Uint128::from(TOKEN_DECIMALS)),
            )],
        ),
        (
            &String::from("asset0000"),
            &[(
                &String::from(MOCK_CONTRACT_ADDR),
                &Uint128::new(200u128).mul(Uint128::from(TOKEN_DECIMALS)),
            )],
        ),
    ]);

    let msg = ExecuteMsg::ProvideLiquidity {
        assets: [
            Asset {
                info: AssetInfo::Token {
                    contract_addr: Addr::unchecked("asset0000"),
                },
                amount: Uint128::from(100u128).mul(Uint128::from(TOKEN_DECIMALS)),
            },
            Asset {
                info: AssetInfo::NativeToken {
                    denom: "uusd".to_string(),
                },
                amount: Uint128::from(200u128).mul(Uint128::from(TOKEN_DECIMALS)),
            },
        ],
        receiver: None,
    };

    let env = mock_env();
    let info = mock_info(
        "addr0000",
        &[Coin {
            denom: "uusd".to_string(),
            amount: Uint128::from(200u128).mul(Uint128::from(TOKEN_DECIMALS)),
        }],
    );

    let res = execute(deps.as_mut(), env, info, msg).unwrap();
    let transfer_from_msg = res.messages.get(0).expect("no message");
    let mint_msg = res.messages.get(1).expect("no message");
    assert_eq!(
        transfer_from_msg,
        &SubMsg {
            msg: WasmMsg::Execute {
                contract_addr: String::from("asset0000"),
                msg: to_binary(&Cw20ExecuteMsg::TransferFrom {
                    owner: String::from("addr0000"),
                    recipient: String::from(MOCK_CONTRACT_ADDR),
                    amount: Uint128::from(100u128).mul(Uint128::from(TOKEN_DECIMALS)),
                })
                .unwrap(),
                funds: vec![],
            }
            .into(),
            id: 0,
            gas_limit: None,
            reply_on: ReplyOn::Never,
        }
    );
    assert_eq!(
        mint_msg,
        &SubMsg {
            msg: WasmMsg::Execute {
                contract_addr: String::from("liquidity0000"),
                msg: to_binary(&Cw20ExecuteMsg::Mint {
                    recipient: String::from("addr0000"),
                    amount: Uint128::from(74_981956874579206461u128),
                })
                .unwrap(),
                funds: vec![],
            }
            .into(),
            id: 0,
            gas_limit: None,
            reply_on: ReplyOn::Never,
        }
    );

    // Check wrong argument
    let msg = ExecuteMsg::ProvideLiquidity {
        assets: [
            Asset {
                info: AssetInfo::Token {
                    contract_addr: Addr::unchecked("asset0000"),
                },
                amount: Uint128::from(100u128).mul(Uint128::from(TOKEN_DECIMALS)),
            },
            Asset {
                info: AssetInfo::NativeToken {
                    denom: "uusd".to_string(),
                },
                amount: Uint128::from(50_000000000000000000u128),
            },
        ],
        receiver: None,
    };

    let env = mock_env();
    let info = mock_info(
        "addr0000",
        &[Coin {
            denom: "uusd".to_string(),
            amount: Uint128::from(100u128).mul(Uint128::from(TOKEN_DECIMALS)),
        }],
    );
    let res = execute(deps.as_mut(), env, info, msg).unwrap_err();
    match res {
        ContractError::Std(StdError::GenericErr { msg, .. }) => assert_eq!(
            msg,
            "Native token balance mismatch between the argument and the transferred".to_string()
        ),
        _ => panic!("Must return generic error"),
    }
}

#[test]
fn test_provide_liquidity_with_minimum_liquidity() {
    let mut deps = mock_dependencies(&[Coin {
        denom: "uusd".to_string(),
        amount: Uint128::new(0u128).mul(Uint128::new(TOKEN_DECIMALS)),
    }]);

    deps.querier.with_token_balances(&[
        (
            &String::from("asset0000"),
            &[(&String::from(MOCK_CONTRACT_ADDR), &Uint128::new(0))],
        ),
        (
            &String::from("liquidity0000"),
            &[(&String::from(MOCK_CONTRACT_ADDR), &Uint128::new(0))],
        ),
    ]);

    let msg = InstantiateMsg {
        asset_infos: [
            AssetInfo::NativeToken {
                denom: "uusd".to_string(),
            },
            AssetInfo::Token {
                contract_addr: Addr::unchecked("asset0000"),
            },
        ],
        token_code_id: 10u64,
        factory_addr: String::from("factory"),
        init_params: Some(to_binary(&StablePoolParams { amp: 1 }).unwrap()), // add minimum amp
    };

    let env = mock_env();
    let info = mock_info("addr0000", &[]);

    let _res = instantiate(deps.as_mut(), env, info, msg).unwrap();
    store_liquidity_token(deps.as_mut(), 1, "liquidity0000".to_string());

    // success case
    deps.querier.with_balance(&[(
        &String::from(MOCK_CONTRACT_ADDR),
        &[Coin {
            denom: "uusd".to_string(),
            amount: Uint128::new(100u128 + 36u128).mul(Uint128::from(TOKEN_DECIMALS)),
        }],
    )]);

    deps.querier.with_token_balances(&[
        (
            &String::from("liquidity0000"),
            &[(
                &String::from(MOCK_CONTRACT_ADDR),
                &Uint128::new(100u128).mul(Uint128::from(TOKEN_DECIMALS)),
            )],
        ),
        (
            &String::from("asset0000"),
            &[(
                &String::from(MOCK_CONTRACT_ADDR),
                &Uint128::new(150u128).mul(Uint128::from(TOKEN_DECIMALS)),
            )],
        ),
    ]);

    let msg = ExecuteMsg::ProvideLiquidity {
        assets: [
            Asset {
                info: AssetInfo::Token {
                    contract_addr: Addr::unchecked("asset0000"),
                },
                amount: Uint128::from(54u128).mul(Uint128::from(TOKEN_DECIMALS)),
            },
            Asset {
                info: AssetInfo::NativeToken {
                    denom: "uusd".to_string(),
                },
                amount: Uint128::from(36u128).mul(Uint128::from(TOKEN_DECIMALS)),
            },
        ],
        receiver: None,
    };

    let env = mock_env();
    let info = mock_info(
        "addr0000",
        &[Coin {
            denom: "uusd".to_string(),
            amount: Uint128::from(36u128).mul(Uint128::from(TOKEN_DECIMALS)),
        }],
    );

    let res = execute(deps.as_mut(), env, info, msg).unwrap();
    assert_eq!(res.attributes[0], attr("action", "provide_liquidity"));

    // invalid provide liquidity operation, we should provide at least minimum liquidity.
    deps.querier.with_balance(&[(
        &String::from(MOCK_CONTRACT_ADDR),
        &[Coin {
            denom: "uusd".to_string(),
            amount: Uint128::new(100u128 + 20u128).mul(Uint128::from(TOKEN_DECIMALS)),
        }],
    )]);

    deps.querier.with_token_balances(&[
        (
            &String::from("liquidity0000"),
            &[(
                &String::from(MOCK_CONTRACT_ADDR),
                &Uint128::new(100u128).mul(Uint128::from(TOKEN_DECIMALS)),
            )],
        ),
        (
            &String::from("asset0000"),
            &[(
                &String::from(MOCK_CONTRACT_ADDR),
                &Uint128::new(150u128).mul(Uint128::from(TOKEN_DECIMALS)),
            )],
        ),
    ]);

    let msg = ExecuteMsg::ProvideLiquidity {
        assets: [
            Asset {
                info: AssetInfo::Token {
                    contract_addr: Addr::unchecked("asset0000"),
                },
                amount: Uint128::from(54u128).mul(Uint128::from(TOKEN_DECIMALS)),
            },
            Asset {
                info: AssetInfo::NativeToken {
                    denom: "uusd".to_string(),
                },
                amount: Uint128::from(20u128).mul(Uint128::from(TOKEN_DECIMALS)),
            },
        ],
        receiver: None,
    };

    let env = mock_env();
    let info = mock_info(
        "addr0000",
        &[Coin {
            denom: "uusd".to_string(),
            amount: Uint128::from(20u128).mul(Uint128::from(TOKEN_DECIMALS)),
        }],
    );

    let res = execute(deps.as_mut(), env, info, msg).unwrap();
    assert_eq!(res.attributes[0], attr("action", "provide_liquidity"));
    assert_eq!(res.attributes[3], attr("assets", "54000000000000000000asset0000, 20000000000000000000uusd"));
    assert_eq!(res.attributes[4], attr("deposits_evaluated", "20000000000000000000uusd, 30000000000000000000asset0000"));
}

/// 유동성 회수 테스트
/// - 두 코인 & 토큰에 대해서 transfer 메시지가 발생했는가
/// - LP 토큰의 Burn 메시지가 발생했는가
/// - 출금한 유동성의 공급량 계산
/// - 환불한 금액에 대한 이벤트 로그
#[test]
fn withdraw_liquidity() {
    let mut deps = mock_dependencies(&[Coin {
        denom: "uusd".to_string(),
        amount: Uint128::new(100u128),
    }]);

    deps.querier.with_token_balances(&[
        (
            &String::from("liquidity0000"),
            &[(&String::from("addr0000"), &Uint128::new(100u128))],
        ),
        (
            &String::from("asset0000"),
            &[(&String::from(MOCK_CONTRACT_ADDR), &Uint128::new(100u128))],
        ),
    ]);

    let msg = InstantiateMsg {
        asset_infos: [
            AssetInfo::NativeToken {
                denom: "uusd".to_string(),
            },
            AssetInfo::Token {
                contract_addr: Addr::unchecked("asset0000"),
            },
        ],
        token_code_id: 10u64,
        factory_addr: String::from("factory"),
        init_params: Some(to_binary(&StablePoolParams { amp: 100 }).unwrap()),
    };

    let env = mock_env();
    let info = mock_info("addr0000", &[]);

    // We can just call .unwrap() to assert this was a success
    let _res = instantiate(deps.as_mut(), env, info, msg).unwrap();

    // Store the liquidity token
    store_liquidity_token(deps.as_mut(), 1, "liquidity0000".to_string());

    // Withdraw liquidity
    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: String::from("addr0000"),
        msg: to_binary(&Cw20HookMsg::WithdrawLiquidity {}).unwrap(),
        amount: Uint128::new(100u128),
    });

    let env = mock_env();
    let info = mock_info("liquidity0000", &[]);
    let res = execute(deps.as_mut(), env, info, msg).unwrap();
    let log_withdrawn_share = res.attributes.get(2).expect("no log");
    let log_refund_assets = res.attributes.get(3).expect("no log");
    let msg_refund_0 = res.messages.get(0).expect("no message");
    let msg_refund_1 = res.messages.get(1).expect("no message");
    let msg_burn_liquidity = res.messages.get(2).expect("no message");
    assert_eq!(
        msg_refund_0,
        &SubMsg {
            msg: CosmosMsg::Bank(BankMsg::Send {
                to_address: String::from("addr0000"),
                amount: vec![Coin {
                    denom: "uusd".to_string(),
                    amount: Uint128::from(100u128),
                }],
            }),
            id: 0,
            gas_limit: None,
            reply_on: ReplyOn::Never,
        }
    );
    assert_eq!(
        msg_refund_1,
        &SubMsg {
            msg: WasmMsg::Execute {
                contract_addr: String::from("asset0000"),
                msg: to_binary(&Cw20ExecuteMsg::Transfer {
                    recipient: String::from("addr0000"),
                    amount: Uint128::from(100u128),
                })
                .unwrap(),
                funds: vec![],
            }
            .into(),
            id: 0,
            gas_limit: None,
            reply_on: ReplyOn::Never,
        }
    );
    assert_eq!(
        msg_burn_liquidity,
        &SubMsg {
            msg: WasmMsg::Execute {
                contract_addr: String::from("liquidity0000"),
                msg: to_binary(&Cw20ExecuteMsg::Burn {
                    amount: Uint128::from(100u128),
                })
                .unwrap(),
                funds: vec![],
            }
            .into(),
            id: 0,
            gas_limit: None,
            reply_on: ReplyOn::Never,
        }
    );

    assert_eq!(
        log_withdrawn_share,
        &attr("withdrawn_share", 100u128.to_string())
    );
    assert_eq!(
        log_refund_assets,
        &attr("refund_assets", "100uusd, 100asset0000")
    );
}

/// native를 token으로 스왑 해보기
#[test]
fn try_native_to_token() {
    let total_share = Uint128::new(30000000000u128);
    let asset_pool_amount = Uint128::new(20000000000u128);
    let collateral_pool_amount = Uint128::new(30000000000u128);
    let offer_amount = Uint128::new(1500000000u128);

    let mut deps = mock_dependencies(&[Coin {
        denom: "uusd".to_string(),
        amount: collateral_pool_amount + offer_amount, /* user deposit must be pre-applied */
    }]);

    deps.querier.with_token_balances(&[
        (
            &String::from("liquidity0000"),
            &[(&String::from(MOCK_CONTRACT_ADDR), &total_share)],
        ),
        (
            &String::from("asset0000"),
            &[(&String::from(MOCK_CONTRACT_ADDR), &asset_pool_amount)],
        ),
    ]);

    let msg = InstantiateMsg {
        asset_infos: [
            AssetInfo::NativeToken {
                denom: "uusd".to_string(),
            },
            AssetInfo::Token {
                contract_addr: Addr::unchecked("asset0000"),
            },
        ],
        token_code_id: 10u64,
        factory_addr: String::from("factory"),
        init_params: Some(to_binary(&StablePoolParams { amp: 100 }).unwrap()),
    };

    let env = mock_env_with_block_time(100);
    let info = mock_info("addr0000", &[]);

    // We can just call .unwrap() to assert this was a success
    let _res = instantiate(deps.as_mut(), env, info, msg).unwrap();

    // Store the liquidity token
    store_liquidity_token(deps.as_mut(), 1, "liquidity0000".to_string());

    // Normal swap
    let msg = ExecuteMsg::Swap {
        offer_asset: Asset {
            info: AssetInfo::NativeToken {
                denom: "uusd".to_string(),
            },
            amount: offer_amount,
        },
        belief_price: None,
        max_spread: Some(Decimal::percent(50)),
        to: None,
    };

    let env = mock_env_with_block_time(1000);
    let info = mock_info(
        "addr0000",
        &[Coin {
            denom: "uusd".to_string(),
            amount: offer_amount,
        }],
    );

    let res = execute(deps.as_mut(), env.clone(), info, msg).unwrap();
    let msg_transfer = res.messages.get(0).expect("no message");

    // Check simulation result
    deps.querier.with_balance(&[(
        &String::from(MOCK_CONTRACT_ADDR),
        &[Coin {
            denom: "uusd".to_string(),
            amount: collateral_pool_amount, /* user deposit must be pre-applied */
        }],
    )]);

    let simulation_res: SimulationResponse = query_simulation(
        deps.as_ref(),
        env,
        Asset {
            info: AssetInfo::NativeToken {
                denom: "uusd".to_string(),
            },
            amount: offer_amount,
        },
    )
    .unwrap();

    assert_eq!(
        res.attributes,
        vec![
            attr("action", "swap"),
            attr("sender", "addr0000"),
            attr("receiver", "addr0000"),
            attr("offer_asset", "uusd"),
            attr("ask_asset", "asset0000"),
            attr("offer_amount", offer_amount.to_string()),
            attr("return_amount", simulation_res.return_amount.to_string()),
            attr("spread_amount", simulation_res.spread_amount.to_string()),
            attr(
                "commission_amount",
                simulation_res.commission_amount.to_string()
            ),
        ]
    );

    assert_eq!(
        &SubMsg {
            msg: WasmMsg::Execute {
                contract_addr: String::from("asset0000"),
                msg: to_binary(&Cw20ExecuteMsg::Transfer {
                    recipient: String::from("addr0000"),
                    amount: simulation_res.return_amount,
                })
                .unwrap(),
                funds: vec![],
            }
            .into(),
            id: 0,
            gas_limit: None,
            reply_on: ReplyOn::Never,
        },
        msg_transfer,
    );
}

/// token을 native로 스왑 해보기
#[test]
fn try_token_to_native() {
    let total_share = Uint128::new(30000000000u128);
    let asset_pool_amount = Uint128::new(20000000000u128);
    let collateral_pool_amount = Uint128::new(30000000000u128);
    let offer_amount = Uint128::new(1500000000u128);

    let mut deps = mock_dependencies(&[Coin {
        denom: "uusd".to_string(),
        amount: collateral_pool_amount,
    }]);

    deps.querier.with_token_balances(&[
        (
            &String::from("liquidity0000"),
            &[(&String::from(MOCK_CONTRACT_ADDR), &total_share)],
        ),
        (
            &String::from("asset0000"),
            &[(
                &String::from(MOCK_CONTRACT_ADDR),
                &(asset_pool_amount + offer_amount),
            )],
        ),
    ]);

    let msg = InstantiateMsg {
        asset_infos: [
            AssetInfo::NativeToken {
                denom: "uusd".to_string(),
            },
            AssetInfo::Token {
                contract_addr: Addr::unchecked("asset0000"),
            },
        ],
        token_code_id: 10u64,
        factory_addr: String::from("factory"),
        init_params: Some(to_binary(&StablePoolParams { amp: 100 }).unwrap()),
    };

    let env = mock_env_with_block_time(100);
    let info = mock_info("addr0000", &[]);

    // We can just call .unwrap() to assert this was a success
    let _res = instantiate(deps.as_mut(), env, info, msg).unwrap();

    // Store the liquidity token
    store_liquidity_token(deps.as_mut(), 1, "liquidity0000".to_string());

    // Unauthorized access; can not execute swap directy for token swap
    let msg = ExecuteMsg::Swap {
        offer_asset: Asset {
            info: AssetInfo::Token {
                contract_addr: Addr::unchecked("asset0000"),
            },
            amount: offer_amount,
        },
        belief_price: None,
        max_spread: None,
        to: None,
    };
    let env = mock_env_with_block_time(1000);
    let info = mock_info("addr0000", &[]);
    let res = execute(deps.as_mut(), env, info, msg).unwrap_err();
    assert_eq!(res, ContractError::Unauthorized {});

    // Normal sell
    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: String::from("addr0000"),
        amount: offer_amount,
        msg: to_binary(&Cw20HookMsg::Swap {
            belief_price: None,
            max_spread: None,
            to: None,
        })
        .unwrap(),
    });
    let env = mock_env_with_block_time(1000);
    let info = mock_info("asset0000", &[]);

    let res = execute(deps.as_mut(), env.clone(), info, msg).unwrap();
    let msg_transfer = res.messages.get(0).expect("no message");

    // Check simulation result
    // Return asset token balance as normal
    deps.querier.with_token_balances(&[
        (
            &String::from("liquidity0000"),
            &[(&String::from(MOCK_CONTRACT_ADDR), &total_share)],
        ),
        (
            &String::from("asset0000"),
            &[(&String::from(MOCK_CONTRACT_ADDR), &(asset_pool_amount))],
        ),
    ]);

    let simulation_res: SimulationResponse = query_simulation(
        deps.as_ref(),
        env,
        Asset {
            amount: offer_amount,
            info: AssetInfo::Token {
                contract_addr: Addr::unchecked("asset0000"),
            },
        },
    )
    .unwrap();

    assert_eq!(
        res.attributes,
        vec![
            attr("action", "swap"),
            attr("sender", "addr0000"),
            attr("receiver", "addr0000"),
            attr("offer_asset", "asset0000"),
            attr("ask_asset", "uusd"),
            attr("offer_amount", offer_amount.to_string()),
            attr("return_amount", simulation_res.return_amount.to_string()),
            attr("spread_amount", simulation_res.spread_amount.to_string()),
            attr(
                "commission_amount",
                simulation_res.commission_amount.to_string()
            ),
        ]
    );

    assert_eq!(
        &SubMsg {
            msg: CosmosMsg::Bank(BankMsg::Send {
                to_address: String::from("addr0000"),
                amount: vec![Coin {
                    denom: "uusd".to_string(),
                    amount: simulation_res.return_amount,
                }],
            }),
            id: 0,
            gas_limit: None,
            reply_on: ReplyOn::Never,
        },
        msg_transfer,
    );

    // Failed due to non asset token contract being used in a swap
    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: String::from("addr0000"),
        amount: offer_amount,
        msg: to_binary(&Cw20HookMsg::Swap {
            belief_price: None,
            max_spread: None,
            to: None,
        })
        .unwrap(),
    });
    let env = mock_env_with_block_time(1000);
    let info = mock_info("liquidtity0000", &[]);
    let res = execute(deps.as_mut(), env, info, msg).unwrap_err();
    assert_eq!(res, ContractError::Unauthorized {});
}

/// max spread가 넘으면 스왑되지 않는 로직 부분 테스트
#[test]
fn test_max_spread() {
    // belief_price - 코인 A / B 사이의 내가 생각한 가격
    // 토큰 B는, 1200개 토큰 A 정도로 생각한다는 의미

    assert_max_spread(
        Some(Decimal::from_ratio(1200u128, 1u128)),
        Some(Decimal::percent(1)),
        Uint128::from(1200000000u128),
        Uint128::from(989999u128),
        Uint128::zero(),
    )
    .unwrap_err();

    assert_max_spread(
        Some(Decimal::from_ratio(1200u128, 1u128)),
        Some(Decimal::percent(1)),
        Uint128::from(1200000000u128),
        Uint128::from(990000u128),
        Uint128::zero(),
    )
    .unwrap();

    assert_max_spread(
        None,
        Some(Decimal::percent(1)),
        Uint128::zero(),
        Uint128::from(989999u128),
        Uint128::from(10001u128),
    )
    .unwrap_err();

    assert_max_spread(
        None,
        Some(Decimal::percent(1)),
        Uint128::zero(),
        Uint128::from(990000u128),
        Uint128::from(10000u128),
    )
    .unwrap();
}

/// 페어 풀 정보 조회
#[test]
fn test_query_pool() {
    let total_share_amount = Uint128::from(111u128);
    let asset_0_amount = Uint128::from(222u128);
    let asset_1_amount = Uint128::from(333u128);
    let mut deps = mock_dependencies(&[Coin {
        denom: "uusd".to_string(),
        amount: asset_0_amount,
    }]);

    deps.querier.with_token_balances(&[
        (
            &String::from("asset0000"),
            &[(&String::from(MOCK_CONTRACT_ADDR), &asset_1_amount)],
        ),
        (
            &String::from("liquidity0000"),
            &[(&String::from(MOCK_CONTRACT_ADDR), &total_share_amount)],
        ),
    ]);

    let msg = InstantiateMsg {
        asset_infos: [
            AssetInfo::NativeToken {
                denom: "uusd".to_string(),
            },
            AssetInfo::Token {
                contract_addr: Addr::unchecked("asset0000"),
            },
        ],
        token_code_id: 10u64,
        factory_addr: String::from("factory"),
        init_params: Some(to_binary(&StablePoolParams { amp: 100 }).unwrap()),
    };

    let env = mock_env();
    let info = mock_info("addr0000", &[]);
    // We can just call .unwrap() to assert this was a success
    let _res = instantiate(deps.as_mut(), env, info, msg).unwrap();

    // Store the liquidity token
    store_liquidity_token(deps.as_mut(), 1, "liquidity0000".to_string());

    let res: PoolResponse = query_pool(deps.as_ref()).unwrap();

    assert_eq!(
        res.assets,
        [
            Asset {
                info: AssetInfo::NativeToken {
                    denom: "uusd".to_string(),
                },
                amount: asset_0_amount
            },
            Asset {
                info: AssetInfo::Token {
                    contract_addr: Addr::unchecked("asset0000"),
                },
                amount: asset_1_amount
            }
        ]
    );
    assert_eq!(res.total_supply, total_share_amount);
}

/// share 정보 조회
#[test]
fn test_query_share() {
    let total_share_amount = Uint128::from(500u128);
    let asset_0_amount = Uint128::from(250u128);
    let asset_1_amount = Uint128::from(1000u128);
    let mut deps = mock_dependencies(&[Coin {
        denom: "uusd".to_string(),
        amount: asset_0_amount,
    }]);

    deps.querier.with_token_balances(&[
        (
            &String::from("asset0000"),
            &[(&String::from(MOCK_CONTRACT_ADDR), &asset_1_amount)],
        ),
        (
            &String::from("liquidity0000"),
            &[(&String::from(MOCK_CONTRACT_ADDR), &total_share_amount)],
        ),
    ]);

    let msg = InstantiateMsg {
        asset_infos: [
            AssetInfo::NativeToken {
                denom: "uusd".to_string(),
            },
            AssetInfo::Token {
                contract_addr: Addr::unchecked("asset0000"),
            },
        ],
        token_code_id: 10u64,
        factory_addr: String::from("factory"),
        init_params: Some(to_binary(&StablePoolParams { amp: 100 }).unwrap()),
    };

    let env = mock_env();
    let info = mock_info("addr0000", &[]);
    // We can just call .unwrap() to assert this was a success
    let _res = instantiate(deps.as_mut(), env, info, msg).unwrap();

    // Store the liquidity token
    store_liquidity_token(deps.as_mut(), 1, "liquidity0000".to_string());

    let res = query_share(deps.as_ref(), Uint128::new(250)).unwrap();

    assert_eq!(res[0].amount, Uint128::new(125));
    assert_eq!(res[1].amount, Uint128::new(500));
}

/// TWAP price 계산
#[test]
fn test_accumulate_prices() {
    struct Case {
        block_time: u64,
        block_time_last: u64,
        last0: u128,
        last1: u128,
        x_amount: u128,
        y_amount: u128,
    }

    struct Result {
        block_time_last: u64,
        cumulative_price_x: u128,
        cumulative_price_y: u128,
        is_some: bool,
    }

    let price_precision = 10u128.pow(TWAP_PRECISION.into());

    let test_cases: Vec<(Case, Result)> = vec![
        (
            Case {
                block_time: 1000,
                block_time_last: 0,
                last0: 0,
                last1: 0,
                x_amount: 250_000000,
                y_amount: 500_000000,
            },
            Result {
                block_time_last: 1000,
                cumulative_price_x: 1008,
                cumulative_price_y: 991,
                is_some: true,
            },
        ),
        // Same block height, no changes
        (
            Case {
                block_time: 1000,
                block_time_last: 1000,
                last0: price_precision,
                last1: 2 * price_precision,
                x_amount: 250_000000,
                y_amount: 500_000000,
            },
            Result {
                block_time_last: 1000,
                cumulative_price_x: 1,
                cumulative_price_y: 2,
                is_some: false,
            },
        ),
        (
            Case {
                block_time: 1500,
                block_time_last: 1000,
                last0: 500 * price_precision,
                last1: 2000 * price_precision,
                x_amount: 250_000000,
                y_amount: 500_000000,
            },
            Result {
                block_time_last: 1500,
                cumulative_price_x: 1004,
                cumulative_price_y: 2495,
                is_some: true,
            },
        ),
    ];

    for test_case in test_cases {
        let (case, result) = test_case;

        let env = mock_env_with_block_time(case.block_time);
        let config = accumulate_prices(
            env.clone(),
            &Config {
                pair_info: PairInfo {
                    asset_infos: [
                        AssetInfo::NativeToken {
                            denom: "uusd".to_string(),
                        },
                        AssetInfo::Token {
                            contract_addr: Addr::unchecked("asset0000"),
                        },
                    ],
                    contract_addr: Addr::unchecked("pair"),
                    liquidity_token: Addr::unchecked("lp_token"),
                    pair_type: PairType::Stable {},
                },
                factory_addr: Addr::unchecked("factory"),
                block_time_last: case.block_time_last,
                price0_cumulative_last: Uint128::new(case.last0),
                price1_cumulative_last: Uint128::new(case.last1),
                init_amp: 100 * AMP_PRECISION,
                init_amp_time: env.block.time.seconds(),
                next_amp: 100 * AMP_PRECISION,
                next_amp_time: env.block.time.seconds(),
            },
            Uint128::new(case.x_amount),
            6,
            Uint128::new(case.y_amount),
            6,
        )
        .unwrap();

        assert_eq!(result.is_some, config.is_some());

        if let Some(config) = config {
            assert_eq!(config.2, result.block_time_last);
            assert_eq!(
                config.0 / Uint128::from(price_precision),
                Uint128::new(result.cumulative_price_x)
            );
            assert_eq!(
                config.1 / Uint128::from(price_precision),
                Uint128::new(result.cumulative_price_y)
            );
        }
    }
}

#[test]
fn swap_simulation() {
    for (index, tt) in CASES.iter().enumerate() {
        let mut deps = mock_dependencies(&[]);
        let total_share = Uint128::new(
            (U256::from(
                Uint128::new(tt.token_a)
                    .mul(Uint128::new(TEST_SWAP_DECIMALS))
                    .u128(),
            ) * U256::from(
                Uint128::new(tt.token_b)
                    .mul(Uint128::new(TEST_SWAP_DECIMALS))
                    .u128(),
            ))
            .integer_sqrt()
            .as_u128(),
        );

        deps.querier.with_token_balances(&[
            (
                &String::from("liquidity0000"),
                &[(&String::from(MOCK_CONTRACT_ADDR), &total_share)],
            ),
            (
                &String::from("asset0000"),
                &[(
                    &String::from(MOCK_CONTRACT_ADDR),
                    &Uint128::new(tt.token_a).mul(Uint128::new(TEST_SWAP_DECIMALS)),
                )],
            ),
            (
                &String::from("asset0001"),
                &[(
                    &String::from(MOCK_CONTRACT_ADDR),
                    &Uint128::new(tt.token_b).mul(Uint128::new(TEST_SWAP_DECIMALS)),
                )],
            ),
        ]);

        let msg = InstantiateMsg {
            asset_infos: [
                AssetInfo::Token {
                    contract_addr: Addr::unchecked("asset0000"),
                },
                AssetInfo::Token {
                    contract_addr: Addr::unchecked("asset0001"),
                },
            ],
            token_code_id: 10u64,
            factory_addr: String::from("factory"),
            init_params: Some(to_binary(&StablePoolParams { amp: 50 }).unwrap()),
        };

        let env = mock_env_with_block_time(100);
        let info = mock_info("addr0000", &[]);

        // We can just call .unwrap() to assert this was a success
        let _res = instantiate(deps.as_mut(), env, info, msg).unwrap();

        // Store the liquidity token
        store_liquidity_token(deps.as_mut(), 1, "liquidity0000".to_string());

        println!();
        println!("-- swap test case #{} --", index);
        let pool_res = query_pool(deps.as_ref()).unwrap();
        println!(
            "pool liquidity, asset0: {}, asset1: {}",
            pool_res.assets[0].amount, pool_res.assets[1].amount
        );

        deps.querier.with_token_balances(&[
            (
                &String::from("liquidity0000"),
                &[(&String::from(MOCK_CONTRACT_ADDR), &total_share)],
            ),
            (
                &String::from("asset0000"),
                &[(
                    &String::from(MOCK_CONTRACT_ADDR),
                    &Uint128::new(tt.token_a + tt.swap_amount)
                        .mul(Uint128::new(TEST_SWAP_DECIMALS)),
                )],
            ),
            (
                &String::from("asset0001"),
                &[(
                    &String::from(MOCK_CONTRACT_ADDR),
                    &Uint128::new(tt.token_b).mul(Uint128::new(TEST_SWAP_DECIMALS)),
                )],
            ),
        ]);

        let result = swap_token(deps.as_mut(), Uint128::from(tt.swap_amount));
        println!("expected amount: {}", tt.token_b - tt.after_token_b);
        println!("{}", result);
    }
}
