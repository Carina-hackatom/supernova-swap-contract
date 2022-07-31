import { SigningCosmWasmClient } from "@cosmjs/cosmwasm-stargate";
import * as config from "../config.js";
import { NativeAsset, newSenderClient, TokenAsset } from "../helpers.js";
import { Factory, Pair, Token } from "../lib/library.js";

// global variables;
let client: SigningCosmWasmClient;
let address: string;
let pair: Pair;
let factory: Factory;
let token: Token;

async function main() {
    await init_configs();

    console.log();
    await query_pair_info();

    console.log();
    await provide_liquidity();

    console.log();
    await swap_token();

    console.log();
    await swap_native();

    console.log();
    await withdraw_liquidity();
}

async function init_configs() {
    console.log("1. init config");
    let [_client, _address] = await newSenderClient();
    client = _client;
    address = _address;

    pair = new Pair(client, address, config.PAIR_CODE_ID, config.POOL_CONTRACT);
    factory = new Factory(client, address, config.FACTORY_CODE_ID, config.FACTORY_CONTRACT);
    token = new Token(client, address, config.TOKEN_CODE_ID, config.TOKEN_A_CONTRACT);
}

async function query_pair_info() {
    console.log("2. query pair info");
    let result = await pair.queryPairInfo();
    console.log(result);
}

async function provide_liquidity() {
    console.log("3. provide liquidity");
    await _fetch_all_balances();

    let allowance = await token.allowance(address, config.POOL_CONTRACT);
    let amount = String(1_000_000000);
    if(BigInt(allowance.allowance) < BigInt(amount)) {
        console.log("current allowance of pool contract: ");
        console.log(allowance);

        console.log();
        console.log("sent trasnaction to increase allowance.");
        let tx = await token.approve(config.POOL_CONTRACT, amount);
    }

    await pair.provideLiquidity(new TokenAsset(config.TOKEN_A_CONTRACT, String(1_000_000000)), new NativeAsset(config.BASE_DENOM, amount));
    console.log(`provided liquidity to ${config.POOL_CONTRACT}`);
    
    await _fetch_all_balances();
}

async function swap_token() {
    console.log("4. swap token");
    let amount = String(100_000000);
    await _fetch_all_balances();

    let tx = await pair.swapToken(new TokenAsset(config.TOKEN_A_CONTRACT, amount));

    await pair.debugSwapLog(tx);

    await _fetch_all_balances();
}

async function swap_native() {
    console.log("5. swap native");
    let amount = String(100_000000);
    await _fetch_all_balances();

    let tx = await pair.swapNative(new NativeAsset(config.BASE_DENOM, amount));

    await pair.debugSwapLog(tx);

    await _fetch_all_balances();
}

async function withdraw_liquidity() {
    console.log("6. withdraw liquidity");
    _fetch_all_balances();

    let lp_token = new Token(client, address, config.TOKEN_CODE_ID, config.LP_TOKEN_ADDR);
    let lp_balance = await lp_token.balanceOf(address);
    await pair.withdrawLiquidity(config.LP_TOKEN_ADDR, lp_balance.balance);

    _fetch_all_balances();
}

async function _fetch_all_balances() {
    // asset a = cw20 token
    // asset b = native coin
    console.log();

    let signer_asset_a = await token.balanceOf(address);
    let signer_asset_b = await client.getBalance(address, config.BASE_DENOM);
    let lp_token = new Token(client, address, config.TOKEN_CODE_ID, config.LP_TOKEN_ADDR);
    let lp_balance = await lp_token.balanceOf(address);
    console.log(`[signer balance] asset a: ${signer_asset_a.balance}, asset b: ${signer_asset_b.amount}, lp_token: ${lp_balance.balance}`);

    let pool_asset_a = await token.balanceOf(config.POOL_CONTRACT);
    let pool_asset_b = await client.getBalance(config.POOL_CONTRACT, config.BASE_DENOM);
    console.log(`[pool balance] asset a: ${pool_asset_a.balance}, asset b: ${pool_asset_b.amount}`);

    console.log();
}

main().catch(console.error);