use crate::error::ContractError;
use crate::math::{
    calc_ask_amount, calc_offer_amount, AMP_PRECISION, MAX_AMP, MAX_AMP_CHANGE,
    MIN_AMP_CHANGING_TIME,
};
use crate::state::{Config, CONFIG};
use cosmwasm_std::{
    to_binary, Addr, CosmosMsg, Decimal, Deps, DepsMut, Env, StdResult, Uint128, WasmMsg,
};
use cw20::Cw20ExecuteMsg;
use novaswap::asset::Asset;
use novaswap::pair::{DEFAULT_SLIPPAGE, MAX_ALLOWED_SLIPPAGE, TWAP_PRECISION};
use novaswap::querier::query_supply;
use std::cmp::Ordering;
use std::str::FromStr;

const GLOBAL_FEE_NUMERATOR: u128 = 30;
const GLOBAL_FEE_DENOMINATOR: u128 = 10000;

// Returns global fee rate
pub fn compute_global_fee() -> Decimal {
    Decimal::from_ratio(
        Uint128::new(GLOBAL_FEE_NUMERATOR),
        Uint128::new(GLOBAL_FEE_DENOMINATOR),
    )
}

// Returns an amount of offer assets for a specified amount of ask assets.
pub fn compute_offer_amount(
    offer_pool: Uint128,
    offer_precision: u8,
    ask_pool: Uint128,
    ask_precision: u8,
    ask_amount: Uint128,
    commission_rate: Decimal,
    amp: u64,
) -> StdResult<(Uint128, Uint128, Uint128)> {
    // ask => offer

    let greater_precision = offer_precision.max(ask_precision);
    let offer_pool = adjust_precision(offer_pool, offer_precision, greater_precision)?;
    let ask_pool = adjust_precision(ask_pool, ask_precision, greater_precision)?;
    let ask_amount = adjust_precision(ask_amount, ask_precision, greater_precision)?;

    let one_minus_commission = Decimal::one() - commission_rate;
    let inv_one_minus_commission: Decimal = Decimal::one() / one_minus_commission;
    let before_commission_deduction = ask_amount * inv_one_minus_commission;

    let offer_amount = Uint128::new(
        calc_offer_amount(
            offer_pool.u128(),
            ask_pool.u128(),
            before_commission_deduction.u128(),
            amp,
        )
        .unwrap(),
    );

    // We assume the assets should stay in a 1:1 ratio, so the true exchange rate is 1. Any exchange rate < 1 could be considered the spread
    let spread_amount = offer_amount.saturating_sub(before_commission_deduction);

    let commission_amount = before_commission_deduction * commission_rate;

    let offer_amount = adjust_precision(offer_amount, greater_precision, offer_precision)?;
    let spread_amount = adjust_precision(spread_amount, greater_precision, ask_precision)?;
    let commission_amount = adjust_precision(commission_amount, greater_precision, ask_precision)?;

    Ok((offer_amount, spread_amount, commission_amount))
}

// Returns a [`ContractError`] on failure.
// If `belief_price` and `max_spread` are both specified, we compute a new spread,
// otherwise we just use the swap spread to check `max_spread`.
pub fn assert_max_spread(
    belief_price: Option<Decimal>,
    max_spread: Option<Decimal>,
    offer_amount: Uint128,
    return_amount: Uint128,
    spread_amount: Uint128,
) -> Result<(), ContractError> {
    let default_spread = Decimal::from_str(DEFAULT_SLIPPAGE)?;
    let max_allowed_spread = Decimal::from_str(MAX_ALLOWED_SLIPPAGE)?;

    let max_spread = max_spread.unwrap_or(default_spread);
    if max_spread.gt(&max_allowed_spread) {
        return Err(ContractError::AllowedSpreadAssertion {});
    }

    if let Some(belief_price) = belief_price {
        let expected_return = offer_amount * (Decimal::one() / belief_price);
        let spread_amount = expected_return
            .checked_sub(return_amount)
            .unwrap_or_else(|_| Uint128::zero());

        if return_amount < expected_return
            && Decimal::from_ratio(spread_amount, expected_return) > max_spread
        {
            return Err(ContractError::MaxSpreadAssertion {});
        }
    } else if Decimal::from_ratio(spread_amount, return_amount + spread_amount) > max_spread {
        return Err(ContractError::MaxSpreadAssertion {});
    }

    Ok(())
}

// Start changing the AMP value. Returns a [`ContractError`] on failure, otherwise returns [`Ok`].
pub fn start_changing_amp(
    mut config: Config,
    deps: DepsMut,
    env: Env,
    next_amp: u64,
    next_amp_time: u64,
) -> Result<(), ContractError> {
    if next_amp == 0 || next_amp > MAX_AMP {
        return Err(ContractError::IncorrectAmp {});
    }

    let current_amp = compute_current_amp(&config, &env)?;

    let next_amp_with_precision = next_amp * AMP_PRECISION;

    if next_amp_with_precision * MAX_AMP_CHANGE < current_amp
        || next_amp_with_precision > current_amp * MAX_AMP_CHANGE
    {
        return Err(ContractError::MaxAmpChangeAssertion {});
    }

    let block_time = env.block.time.seconds();

    if block_time < config.init_amp_time + MIN_AMP_CHANGING_TIME
        || next_amp_time < block_time + MIN_AMP_CHANGING_TIME
    {
        return Err(ContractError::MinAmpChangingTimeAssertion {});
    }

    config.init_amp = current_amp;
    config.next_amp = next_amp_with_precision;
    config.init_amp_time = block_time;
    config.next_amp_time = next_amp_time;

    CONFIG.save(deps.storage, &config)?;

    Ok(())
}

// Stop changing the AMP value. Returns [`Ok`].
pub fn stop_changing_amp(mut config: Config, deps: DepsMut, env: Env) -> StdResult<()> {
    let current_amp = compute_current_amp(&config, &env)?;
    let block_time = env.block.time.seconds();

    config.init_amp = current_amp;
    config.next_amp = current_amp;
    config.init_amp_time = block_time;
    config.next_amp_time = block_time;

    // now (block_time < next_amp_time) is always False, so we return the saved AMP
    CONFIG.save(deps.storage, &config)?;

    Ok(())
}

// Compute the current pool amplification coefficient (AMP).
pub fn compute_current_amp(config: &Config, env: &Env) -> StdResult<u64> {
    let block_time = env.block.time.seconds();

    if block_time < config.next_amp_time {
        let elapsed_time =
            Uint128::from(block_time).checked_sub(Uint128::from(config.init_amp_time))?;
        let time_range =
            Uint128::from(config.next_amp_time).checked_sub(Uint128::from(config.init_amp_time))?;
        let init_amp = Uint128::from(config.init_amp);
        let next_amp = Uint128::from(config.next_amp);

        if config.next_amp > config.init_amp {
            let amp_range = next_amp - init_amp;
            let res = init_amp + (amp_range * elapsed_time).checked_div(time_range)?;
            Ok(res.u128() as u64)
        } else {
            let amp_range = init_amp - next_amp;
            let res = init_amp - (amp_range * elapsed_time).checked_div(time_range)?;
            Ok(res.u128() as u64)
        }
    } else {
        Ok(config.next_amp)
    }
}

// adjust given value from `current_precision` to `new_precision`
pub fn adjust_precision(
    value: Uint128,
    current_precision: u8,
    new_precision: u8,
) -> StdResult<Uint128> {
    Ok(match current_precision.cmp(&new_precision) {
        Ordering::Equal => value,
        Ordering::Less => value.checked_mul(Uint128::new(
            // value = value * 10^(new - curr)
            10_u128.pow((new_precision - current_precision) as u32),
        ))?,
        Ordering::Greater => value.checked_div(Uint128::new(
            // value = value / 10^(curr - new)
            10_u128.pow((current_precision - new_precision) as u32),
        ))?,
    })
}

// Mint LP tokens for a beneficiary
pub fn mint_liquidity_token_message(
    config: &Config,
    recipient: Addr,
    amount: Uint128,
) -> Result<CosmosMsg, ContractError> {
    let lp_token = config.pair_info.liquidity_token.clone();
    Ok(CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: lp_token.to_string(),
        msg: to_binary(&Cw20ExecuteMsg::Mint {
            recipient: recipient.to_string(),
            amount,
        })?,
        funds: vec![],
    }))
}

// calculate accumulate prices
pub fn accumulate_prices(
    env: Env,
    config: &Config,
    x: Uint128,
    x_precision: u8,
    y: Uint128,
    y_precision: u8,
) -> StdResult<Option<(Uint128, Uint128, u64)>> {
    let block_time = env.block.time.seconds();
    if block_time <= config.block_time_last {
        return Ok(None);
    }

    let greater_precision = x_precision.max(y_precision).max(TWAP_PRECISION);
    let x = adjust_precision(x, x_precision, greater_precision)?;
    let y = adjust_precision(y, y_precision, greater_precision)?;

    let time_elapsed = Uint128::from(block_time - config.block_time_last);

    let mut pcl0 = config.price0_cumulative_last;
    let mut pcl1 = config.price1_cumulative_last;

    if !x.is_zero() && !y.is_zero() {
        let current_amp = compute_current_amp(config, &env)?;
        pcl0 = config.price0_cumulative_last.wrapping_add(adjust_precision(
            time_elapsed.checked_mul(Uint128::new(
                calc_ask_amount(
                    x.u128(),
                    y.u128(),
                    adjust_precision(Uint128::new(1), 0, greater_precision)?.u128(),
                    current_amp,
                )
                .unwrap(),
            ))?,
            greater_precision,
            TWAP_PRECISION,
        )?);
        pcl1 = config.price1_cumulative_last.wrapping_add(adjust_precision(
            time_elapsed.checked_mul(Uint128::new(
                calc_ask_amount(
                    y.u128(),
                    x.u128(),
                    adjust_precision(Uint128::new(1), 0, greater_precision)?.u128(),
                    current_amp,
                )
                .unwrap(),
            ))?,
            greater_precision,
            TWAP_PRECISION,
        )?)
    };

    Ok(Some((pcl0, pcl1, block_time)))
}

pub fn get_share_in_assets(
    pools: &[Asset; 2],
    amount: Uint128,
    total_share: Uint128,
) -> [Asset; 2] {
    let mut share_ratio = Decimal::zero();
    if !total_share.is_zero() {
        share_ratio = Decimal::from_ratio(amount, total_share);
    }

    [
        Asset {
            info: pools[0].info.clone(),
            amount: pools[0].amount * share_ratio,
        },
        Asset {
            info: pools[1].info.clone(),
            amount: pools[1].amount * share_ratio,
        },
    ]
}

pub fn pool_info(deps: Deps, config: Config) -> StdResult<([Asset; 2], Uint128)> {
    let contract_addr = config.pair_info.contract_addr.clone();
    let pools: [Asset; 2] = config.pair_info.query_pools(&deps.querier, contract_addr)?;
    let total_supply: Uint128 = query_supply(&deps.querier, config.pair_info.liquidity_token)?;

    Ok((pools, total_supply))
}

/// compute swap then returns return_amount, spread_amount and commission_amount.
pub fn compute_swap(
    offer_pool: Uint128,
    offer_precision: u8,
    ask_pool: Uint128,
    ask_precision: u8,
    offer_amount: Uint128,
    commission_rate: Decimal,
    amp: u64,
) -> StdResult<(Uint128, Uint128, Uint128)> {
    // offer => ask

    let greater_precision = offer_precision.max(ask_precision);
    let offer_pool = adjust_precision(offer_pool, offer_precision, greater_precision)?;
    let ask_pool = adjust_precision(ask_pool, ask_precision, greater_precision)?;
    let offer_amount = adjust_precision(offer_amount, offer_precision, greater_precision)?;

    let return_amount = Uint128::new(
        calc_ask_amount(offer_pool.u128(), ask_pool.u128(), offer_amount.u128(), amp).unwrap(),
    );

    // We assume the assets should stay in a 1:1 ratio, so the true exchange rate is 1. So any exchange rate <1 could be considered the spread
    let spread_amount = offer_amount.saturating_sub(return_amount);

    let commission_amount: Uint128 = return_amount * commission_rate;

    // The commission will be absorbed by the pool
    let return_amount: Uint128 = return_amount.checked_sub(commission_amount).unwrap();

    let return_amount = adjust_precision(return_amount, greater_precision, ask_precision)?;
    let spread_amount = adjust_precision(spread_amount, greater_precision, ask_precision)?;
    let commission_amount = adjust_precision(commission_amount, greater_precision, ask_precision)?;

    Ok((return_amount, spread_amount, commission_amount))
}
