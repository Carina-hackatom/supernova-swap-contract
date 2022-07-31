use cosmwasm_std::{attr, entry_point, from_binary, to_binary, Addr, Binary, CosmosMsg, Decimal, Deps, DepsMut, Env, MessageInfo, Reply, ReplyOn, Response, StdError, StdResult, SubMsg, Uint128, WasmMsg, Uint256};
use cw2::set_contract_version;
use cw20::{Cw20ExecuteMsg, Cw20ReceiveMsg, MinterResponse};
use novaswap::asset::{format_lp_token_name, Asset, AssetInfo};
use novaswap::pair::{
    ConfigResponse, CumulativePricesResponse, Cw20HookMsg, ExecuteMsg, InstantiateMsg, MigrateMsg,
    PoolResponse, QueryMsg, ReverseSimulationResponse, SimulationResponse, StablePoolConfig,
    StablePoolParams, StablePoolUpdateParams,
};

use novaswap::pairinfo::{PairInfo, PairType};
use novaswap::token::InstantiateMsg as TokenInstantiateMsg;

use crate::error::ContractError;
use crate::math::{compute_current_amp, compute_d, AMP_PRECISION, MAX_AMP, N_COINS, MINIMUM_AMP};
use crate::state::{Config, CONFIG};

use crate::utils::{
    accumulate_prices, adjust_precision, assert_max_spread, compute_global_fee,
    compute_offer_amount, compute_swap, get_share_in_assets, mint_liquidity_token_message,
    pool_info, start_changing_amp, stop_changing_amp,
};
use novaswap::querier::{query_factory_config, query_supply, query_token_precision};
use novaswap::U256;

const CONTRACT_NAME: &str = "novaswap-pair-stable";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");
const INSTANTIATE_TOKEN_REPLY_ID: u64 = 1;

// instantiate new novaswap pair stable contract.
// this will mint new token represents LP shares.
#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    msg.asset_infos[0].check(deps.api)?;
    msg.asset_infos[1].check(deps.api)?;

    if msg.asset_infos[0] == msg.asset_infos[1] {
        return Err(ContractError::DoublingAssets {});
    }

    if msg.init_params.is_none() {
        return Err(ContractError::InitParamsNotFound {});
    }

    let params: StablePoolParams = from_binary(&msg.init_params.unwrap())?;

    // Zero AMP means to be constant product market making model
    if params.amp == 0 || params.amp > MAX_AMP {
        return Err(ContractError::IncorrectAmp {});
    }

    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    // set config
    let config = Config {
        pair_info: PairInfo {
            contract_addr: env.contract.address.clone(),
            liquidity_token: Addr::unchecked(""),
            asset_infos: msg.asset_infos.clone(),
            pair_type: PairType::Stable {},
        },
        factory_addr: deps.api.addr_validate(msg.factory_addr.as_str())?,
        block_time_last: 0,
        price0_cumulative_last: Uint128::zero(),
        price1_cumulative_last: Uint128::zero(),
        init_amp: params.amp * AMP_PRECISION,
        init_amp_time: env.block.time.seconds(),
        next_amp: params.amp * AMP_PRECISION,
        next_amp_time: env.block.time.seconds(),
    };

    CONFIG.save(deps.storage, &config)?;

    let token_name = format_lp_token_name(msg.asset_infos, &deps.querier)?;

    // Create LP Token
    let sub_msg: Vec<SubMsg> = vec![SubMsg {
        msg: WasmMsg::Instantiate {
            code_id: msg.token_code_id,
            msg: to_binary(&TokenInstantiateMsg {
                name: token_name,
                symbol: "uLP".to_string(),
                decimals: 6,
                initial_balances: vec![],
                mint: Some(MinterResponse {
                    minter: env.contract.address.to_string(),
                    cap: None,
                }),
            })?,
            funds: vec![],
            admin: None,
            label: String::from("Novaswap LP token"),
        }
        .into(),
        id: INSTANTIATE_TOKEN_REPLY_ID,
        gas_limit: None,
        reply_on: ReplyOn::Success,
    }];

    Ok(Response::new().add_submessages(sub_msg))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn reply(deps: DepsMut, _env: Env, msg: Reply) -> Result<Response, ContractError> {
    let mut config: Config = CONFIG.load(deps.storage)?;

    if config.pair_info.liquidity_token != Addr::unchecked("") {
        return Err(ContractError::Unauthorized {});
    }

    let res = cw_utils::parse_reply_instantiate_data(msg).unwrap();

    config.pair_info.liquidity_token = deps.api.addr_validate(res.contract_address.as_str())?;

    CONFIG.save(deps.storage, &config)?;

    Ok(Response::new().add_attribute("liquidity_token_addr", config.pair_info.liquidity_token))
}

pub struct SwapParams {
    offer_asset: Asset,
    belief_price: Option<Decimal>,
    max_spread: Option<Decimal>,
    to: Option<Addr>,
}

// Exposes all the execute functions.
#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::Receive(msg) => receive_cw20(deps, env, info, msg),
        ExecuteMsg::UpdateConfig { params } => update_config(deps, env, info, params),
        ExecuteMsg::Swap {
            offer_asset,
            belief_price,
            max_spread,
            to,
        } => {
            let to_addr = if let Some(addr) = to {
                Some(deps.api.addr_validate(addr.as_str())?)
            } else {
                None
            };

            if !offer_asset.is_native_token() {
                return Err(ContractError::Unauthorized {});
            }

            swap(
                deps,
                env,
                info.clone(),
                info.sender,
                SwapParams {
                    offer_asset,
                    belief_price,
                    max_spread,
                    to: to_addr,
                },
            )
        }
        ExecuteMsg::ProvideLiquidity { assets, receiver } => {
            provide_liquidity(deps, env, info, assets, receiver)
        }
    }
}

fn receive_cw20(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: Cw20ReceiveMsg,
) -> Result<Response, ContractError> {
    let contract_addr = info.sender.clone();

    match from_binary(&msg.msg) {
        Ok(Cw20HookMsg::Swap {
            belief_price,
            max_spread,
            to,
        }) => {
            let mut authorized = false;
            let config: Config = CONFIG.load(deps.storage)?;

            for pool in config.pair_info.asset_infos {
                if let AssetInfo::Token { contract_addr, .. } = &pool {
                    if contract_addr == &info.sender {
                        authorized = true;
                    }
                }
            }

            if !authorized {
                return Err(ContractError::Unauthorized {});
            }

            let to_addr = if let Some(to_addr) = to {
                Some(deps.api.addr_validate(to_addr.as_str())?)
            } else {
                None
            };

            let sender = deps.api.addr_validate(msg.sender.as_str())?;

            swap(
                deps,
                env,
                info,
                sender,
                SwapParams {
                    offer_asset: Asset {
                        info: AssetInfo::Token { contract_addr },
                        amount: msg.amount,
                    },
                    belief_price,
                    max_spread,
                    to: to_addr,
                },
            )
        }
        Ok(Cw20HookMsg::WithdrawLiquidity {}) => {
            withdraw_liquidity(deps, env, info, Addr::unchecked(msg.sender), msg.amount)
        }
        Err(err) => Err(ContractError::Std(err)),
    }
}

fn provide_liquidity(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    assets: [Asset; 2],
    receiver: Option<String>,
) -> Result<Response, ContractError> {
    assets[0].info.check(deps.api)?;
    assets[1].info.check(deps.api)?;

    // check the amount listed in messages are equal to actually received native coin.
    for asset in assets.iter() {
        asset.assert_sent_native_token_balance(&info)?;
    }

    // get pools asset from pair contract
    let mut config: Config = CONFIG.load(deps.storage)?;
    let mut pools: [Asset; 2] = config
        .pair_info
        .query_pools(&deps.querier, env.contract.address.clone())?;

    // get the amount of coins the user want to deposit.
    let mut deposits: [Uint128; 2] = [
        assets
            .iter()
            .find(|a| a.info.equal(&pools[0].info))
            .map(|a| a.amount)
            .expect("Wrong asset info is given"),
        assets
            .iter()
            .find(|a| a.info.equal(&pools[1].info))
            .map(|a| a.amount)
            .expect("Wrong asset info is given"),
    ];

    if deposits[0].is_zero() && deposits[1].is_zero() {
        return Err(ContractError::InvalidZeroAmount {});
    }

    let mut messages: Vec<CosmosMsg> = vec![];
    for (i, pool) in pools.iter_mut().enumerate() {
        if deposits[i].is_zero() && pool.amount.is_zero() {
            return Err(ContractError::InvalidProvideLPsWithSingleToken {});
        }

        if !deposits[i].is_zero() {
            if let AssetInfo::Token { contract_addr } = &pool.info {
                // Add TransferFrom message to messages.
                messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
                    contract_addr: contract_addr.to_string(),
                    msg: to_binary(&Cw20ExecuteMsg::TransferFrom {
                        owner: info.sender.to_string(),
                        recipient: env.contract.address.to_string(),
                        amount: deposits[i],
                    })?,
                    funds: vec![],
                }))
            } else {
                // If the asset is a native token, the pool balance already increased.
                pool.amount = pool.amount.checked_sub(deposits[i])?;
            }
        }
    }

    // assert_provided_with_as_same_ratio
    let amp = compute_current_amp(&config, &env)?;
    if amp == MINIMUM_AMP && !pools[0].amount.is_zero() && !pools[1].amount.is_zero(){
        let reserve_a = Uint256::from(pools[0].amount);
        let reserve_b = Uint256::from(pools[1].amount);
        let amount_a = Uint256::from(deposits[0]);
        let amount_b = Uint256::from(deposits[1]);

        let real_amount_b = reserve_b * amount_a / reserve_a;
        let real_amount_a = reserve_a * amount_b / reserve_b;
        let optimal_amount_b = Uint128::try_from(real_amount_b)?;
        let optimal_amount_a = Uint128::try_from(real_amount_a)?;

        if deposits[1] > optimal_amount_b {
            deposits[1] = optimal_amount_b;
        } else if deposits[0] > optimal_amount_a {
            deposits[0] = optimal_amount_a;
        }
    }

    // Assert that slippage tolerance is respected
    // assert_slippage_tolerance(&slippage_tolerance, &deposits, &pools)?;

    // decimals of each token.
    let token_precision_0 = query_token_precision(&deps.querier, pools[0].info.clone())?;
    let token_precision_1 = query_token_precision(&deps.querier, pools[1].info.clone())?;
    let greater_precision = token_precision_0.max(token_precision_1);

    let deposit_amount_0 = adjust_precision(deposits[0], token_precision_0, greater_precision)?;
    let deposit_amount_1 = adjust_precision(deposits[1], token_precision_1, greater_precision)?;

    // total supply of liquidity token
    let total_supply = query_supply(&deps.querier, config.pair_info.liquidity_token.clone())?;

    let share = if total_supply.is_zero() {
        // If I'm a first liquidity provider.
        let lp_token_precision = query_token_precision(
            &deps.querier,
            AssetInfo::Token {
                contract_addr: config.pair_info.liquidity_token.clone(),
            },
        )?;

        adjust_precision(
            Uint128::new(
                (U256::from(deposit_amount_0.u128()) * U256::from(deposit_amount_1.u128()))
                    .integer_sqrt()
                    .as_u128(),
            ),
            greater_precision,
            lp_token_precision,
        )?
    } else {
        let leverage = compute_current_amp(&config, &env)?
            .checked_mul(u64::from(N_COINS))
            .unwrap();

        let mut pool_amount_0 =
            adjust_precision(pools[0].amount, token_precision_0, greater_precision)?;
        let mut pool_amount_1 =
            adjust_precision(pools[1].amount, token_precision_1, greater_precision)?;

        let d_before_addition_liquidity =
            compute_d(leverage, pool_amount_0.u128(), pool_amount_1.u128()).unwrap();

        pool_amount_0 = pool_amount_0.checked_add(deposit_amount_0)?;
        pool_amount_1 = pool_amount_1.checked_add(deposit_amount_1)?;

        let d_after_addition_liquididty =
            compute_d(leverage, pool_amount_0.u128(), pool_amount_1.u128()).unwrap();

        if d_before_addition_liquidity >= d_after_addition_liquididty {
            return Err(ContractError::LiquidityAmountTooSmall {});
        }

        total_supply.multiply_ratio(
            d_after_addition_liquididty - d_before_addition_liquidity,
            d_before_addition_liquidity,
        )
    };

    if share.is_zero() {
        return Err(ContractError::LiquidityAmountTooSmall {});
    }

    // mint lp token
    let receiver = receiver.unwrap_or_else(|| info.sender.to_string());
    messages.push(mint_liquidity_token_message(
        &config,
        deps.api.addr_validate(receiver.as_str())?,
        share,
    )?);

    // accumulate prices
    if let Some((price0_cumulative_new, price1_cumulative_new, block_time)) = accumulate_prices(
        env,
        &config,
        pools[0].amount,
        token_precision_0,
        pools[1].amount,
        token_precision_1,
    )? {
        config.price0_cumulative_last = price0_cumulative_new;
        config.price1_cumulative_last = price1_cumulative_new;
        config.block_time_last = block_time;
        CONFIG.save(deps.storage, &config)?;
    }

    Ok(Response::new().add_messages(messages).add_attributes(vec![
        attr("action", "provide_liquidity"),
        attr("sender", info.sender.as_str()),
        attr("receiver", receiver.as_str()),
        attr("assets", format!("{}, {}", assets[0], assets[1])),
        attr("deposits_evaluated", format!("{}, {}", format!("{}{}", deposits[0], pools[0].info), format!("{}{}", deposits[1], pools[1].info))),
        attr("share", share.to_string()),
    ]))
}

fn withdraw_liquidity(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    sender: Addr,
    amount: Uint128,
) -> Result<Response, ContractError> {
    let mut config: Config = CONFIG.load(deps.storage).unwrap();

    if info.sender != config.pair_info.liquidity_token {
        return Err(ContractError::Unauthorized {});
    }

    let (pools, total_share) = pool_info(deps.as_ref(), config.clone())?;
    let refund_assets: [Asset; 2] = get_share_in_assets(&pools, amount, total_share);

    // Accumulate prices for the assets in the pool
    if let Some((price0_cumulative_new, price1_cumulative_new, block_time)) = accumulate_prices(
        env,
        &config,
        pools[0].amount,
        query_token_precision(&deps.querier, pools[0].info.clone())?,
        pools[1].amount,
        query_token_precision(&deps.querier, pools[1].info.clone())?,
    )? {
        config.price0_cumulative_last = price0_cumulative_new;
        config.price1_cumulative_last = price1_cumulative_new;
        config.block_time_last = block_time;
        CONFIG.save(deps.storage, &config)?;
    }

    let messages: Vec<CosmosMsg> = vec![
        refund_assets[0].clone().transfer_msg(sender.clone())?,
        refund_assets[1].clone().transfer_msg(sender.clone())?,
        CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: config.pair_info.liquidity_token.to_string(),
            msg: to_binary(&Cw20ExecuteMsg::Burn { amount })?,
            funds: vec![],
        }),
    ];

    let attributes = vec![
        attr("action", "withdraw_liquidity"),
        attr("sender", sender.as_str()),
        attr("withdrawn_share", &amount.to_string()),
        attr(
            "refund_assets",
            format!("{}, {}", refund_assets[0], refund_assets[1]),
        ),
    ];

    Ok(Response::new()
        .add_messages(messages)
        .add_attributes(attributes))
}

/// Update configuration (mainly amplification parameter)
fn update_config(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    params: Binary,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    let factory_config = query_factory_config(&deps.querier, config.factory_addr.clone())?;

    if info.sender != factory_config.owner {
        return Err(ContractError::Unauthorized {});
    }

    match from_binary::<StablePoolUpdateParams>(&params)? {
        StablePoolUpdateParams::StartChangingAmp {
            next_amp,
            next_amp_time,
        } => start_changing_amp(config, deps, env, next_amp, next_amp_time)?,
        StablePoolUpdateParams::StopChangingAmp {} => stop_changing_amp(config, deps, env)?,
    }

    Ok(Response::default())
}

fn swap(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    sender: Addr,
    params: SwapParams,
) -> Result<Response, ContractError> {
    params.offer_asset.assert_sent_native_token_balance(&info)?;

    let mut config: Config = CONFIG.load(deps.storage)?;

    let pools: Vec<Asset> = config
        .pair_info
        .query_pools(&deps.querier, env.clone().contract.address)?
        .into_iter()
        .map(|mut p| {
            if p.info.equal(&params.offer_asset.info) {
                p.amount = p.amount.checked_sub(params.offer_asset.amount).unwrap();
            }

            p
        })
        .collect();

    let offer_pool: Asset;
    let ask_pool: Asset;

    if params.offer_asset.info.equal(&pools[0].info) {
        offer_pool = pools[0].clone();
        ask_pool = pools[1].clone();
    } else if params.offer_asset.info.equal(&pools[1].info) {
        offer_pool = pools[1].clone();
        ask_pool = pools[0].clone();
    } else {
        return Err(ContractError::AssetMismatch {});
    }

    let offer_amount = params.offer_asset.amount;
    let ask_pool_info = ask_pool.info.clone();

    let (return_amount, spread_amount, commission_amount) = compute_swap(
        offer_pool.amount,
        query_token_precision(&deps.querier, offer_pool.info)?,
        ask_pool.amount,
        query_token_precision(&deps.querier, ask_pool.info)?,
        offer_amount,
        compute_global_fee(),
        compute_current_amp(&config, &env)?,
    )?;

    // Check the max spread limit
    assert_max_spread(
        params.belief_price,
        params.max_spread,
        offer_amount,
        return_amount + commission_amount,
        spread_amount,
    )?;

    // Compute tax for the ask asset
    let return_asset = Asset {
        info: ask_pool_info.clone(),
        amount: return_amount,
    };

    let receiver = params.to.unwrap_or_else(|| sender.clone());
    let messages = vec![return_asset.transfer_msg(receiver.clone())?];

    // Accumulate prices for the assets in the pool
    if let Some((price0_cumulative_new, price1_cumulative_new, block_time)) = accumulate_prices(
        env,
        &config,
        pools[0].amount,
        query_token_precision(&deps.querier, pools[0].info.clone())?,
        pools[1].amount,
        query_token_precision(&deps.querier, pools[1].info.clone())?,
    )? {
        config.price0_cumulative_last = price0_cumulative_new;
        config.price1_cumulative_last = price1_cumulative_new;
        config.block_time_last = block_time;
        CONFIG.save(deps.storage, &config)?;
    }

    Ok(Response::new()
        .add_messages(messages)
        .add_attribute("action", "swap")
        .add_attribute("sender", sender.as_str())
        .add_attribute("receiver", receiver.as_str())
        .add_attribute("offer_asset", params.offer_asset.info.to_string())
        .add_attribute("ask_asset", ask_pool_info.to_string())
        .add_attribute("offer_amount", offer_amount.to_string())
        .add_attribute("return_amount", return_amount.to_string())
        .add_attribute("spread_amount", spread_amount.to_string())
        .add_attribute("commission_amount", commission_amount.to_string()))
}

// Exposes all query functions.
#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Config {} => to_binary(&query_config(deps, env)?),
        QueryMsg::Pair {} => to_binary(&query_pair_info(deps)?),
        QueryMsg::Pool {} => to_binary(&query_pool(deps)?),
        QueryMsg::Share { amount } => to_binary(&query_share(deps, amount)?),
        QueryMsg::Simulation { offer_asset } => {
            to_binary(&query_simulation(deps, env, offer_asset)?)
        }
        QueryMsg::ReverseSimulation { ask_asset } => {
            to_binary(&query_reverse_simulation(deps, env, ask_asset)?)
        }
        QueryMsg::CumulativePrices {} => to_binary(&query_cumulative_prices(deps, env)?),
    }
}

// Returns pair info
pub fn query_pair_info(deps: Deps) -> StdResult<PairInfo> {
    let config = CONFIG.load(deps.storage)?;

    Ok(config.pair_info)
}

// Returns the amount of assets in the pair contract as well as the amount of LP.
pub fn query_pool(deps: Deps) -> StdResult<PoolResponse> {
    let config: Config = CONFIG.load(deps.storage)?;
    let (assets, total_supply) = pool_info(deps, config)?;

    Ok(PoolResponse {
        assets,
        total_supply,
    })
}

// Returns the amount of assets owned within the pool with the amount of LP tokens.
pub fn query_share(deps: Deps, amount: Uint128) -> StdResult<[Asset; 2]> {
    let config: Config = CONFIG.load(deps.storage)?;
    let (pools, total_share) = pool_info(deps, config)?;
    let owned_assets = get_share_in_assets(&pools, amount, total_share);

    Ok(owned_assets)
}

// Returns information about a swap simulation
pub fn query_simulation(deps: Deps, env: Env, offer_asset: Asset) -> StdResult<SimulationResponse> {
    let config: Config = CONFIG.load(deps.storage)?;
    let contract_addr = config.pair_info.contract_addr.clone();

    let pools: [Asset; 2] = config.pair_info.query_pools(&deps.querier, contract_addr)?;

    let offer_pool: Asset;
    let ask_pool: Asset;
    if offer_asset.info.equal(&pools[0].info) {
        offer_pool = pools[0].clone();
        ask_pool = pools[1].clone();
    } else if offer_asset.info.equal(&pools[1].info) {
        offer_pool = pools[1].clone();
        ask_pool = pools[0].clone();
    } else {
        return Err(StdError::generic_err(
            "Given offer asset doesn't belong to pairs",
        ));
    }

    let (return_amount, spread_amount, commission_amount) = compute_swap(
        offer_pool.amount,
        query_token_precision(&deps.querier, offer_pool.info)?,
        ask_pool.amount,
        query_token_precision(&deps.querier, ask_pool.info)?,
        offer_asset.amount,
        compute_global_fee(),
        compute_current_amp(&config, &env)?,
    )?;

    Ok(SimulationResponse {
        return_amount,
        spread_amount,
        commission_amount,
    })
}

// Returns information about a reverse swap simulation
pub fn query_reverse_simulation(
    deps: Deps,
    env: Env,
    ask_asset: Asset,
) -> StdResult<ReverseSimulationResponse> {
    let config: Config = CONFIG.load(deps.storage)?;
    let contract_addr = config.pair_info.contract_addr.clone();

    let pools: [Asset; 2] = config.pair_info.query_pools(&deps.querier, contract_addr)?;

    let offer_pool: Asset;
    let ask_pool: Asset;
    if ask_asset.info.equal(&pools[0].info) {
        ask_pool = pools[0].clone();
        offer_pool = pools[1].clone();
    } else if ask_asset.info.equal(&pools[1].info) {
        ask_pool = pools[1].clone();
        offer_pool = pools[0].clone();
    } else {
        return Err(StdError::generic_err(
            "Given ask asset doesn't belong to pairs",
        ));
    }

    let (offer_amount, spread_amount, commission_amount) = compute_offer_amount(
        offer_pool.amount,
        query_token_precision(&deps.querier, offer_pool.info)?,
        ask_pool.amount,
        query_token_precision(&deps.querier, ask_pool.info)?,
        ask_asset.amount,
        compute_global_fee(),
        compute_current_amp(&config, &env)?,
    )?;

    Ok(ReverseSimulationResponse {
        offer_amount,
        spread_amount,
        commission_amount,
    })
}

// Returns price oracle
pub fn query_cumulative_prices(deps: Deps, env: Env) -> StdResult<CumulativePricesResponse> {
    let config: Config = CONFIG.load(deps.storage)?;
    let (assets, total_share) = pool_info(deps, config.clone())?;

    let mut price0_cumulative_last = config.price0_cumulative_last;
    let mut price1_cumulative_last = config.price1_cumulative_last;

    if let Some((price0_cumulative_new, price1_cumulative_new, _)) = accumulate_prices(
        env,
        &config,
        assets[0].amount,
        query_token_precision(&deps.querier, assets[0].info.clone())?,
        assets[1].amount,
        query_token_precision(&deps.querier, assets[1].info.clone())?,
    )? {
        price0_cumulative_last = price0_cumulative_new;
        price1_cumulative_last = price1_cumulative_new;
    }

    let resp = CumulativePricesResponse {
        assets,
        total_share,
        price0_cumulative_last,
        price1_cumulative_last,
    };

    Ok(resp)
}

// Returns current configuration
pub fn query_config(deps: Deps, env: Env) -> StdResult<ConfigResponse> {
    let config = CONFIG.load(deps.storage)?;

    Ok(ConfigResponse {
        block_time_last: config.block_time_last,
        params: Some(to_binary(&StablePoolConfig {
            amp: Decimal::from_ratio(compute_current_amp(&config, &env)?, AMP_PRECISION),
        })?),
    })
}

// Used for contract migration.
#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(_deps: DepsMut, _env: Env, _msg: MigrateMsg) -> Result<Response, ContractError> {
    Ok(Response::default())
}
