import { ExecuteResult, InstantiateResult, SigningCosmWasmClient } from "@cosmjs/cosmwasm-stargate";
import { DEFAULT_FEE, TOKEN_CODE_ID } from "../config.js";
import { NativeAsset, toEncodedBinary, TokenAsset } from "../helpers.js";

export class Factory {
    client: SigningCosmWasmClient
    signer: string
    codeId: number
    contractAddress?: string

    constructor(client: SigningCosmWasmClient, signer: string, codeId: number, contractAddress?: string) {
        this.client = client;
        this.codeId = codeId;
        this.signer = signer;
        if(contractAddress) this.contractAddress = contractAddress;
    }

    async initialize(owner: string, tokenCodeId: number, pairCodeId: number): Promise<InstantiateResult> {
        let msg = {
            owner: owner,
            token_code_id: tokenCodeId,
            pair_configs: [
                {
                    code_id: pairCodeId,
                    pair_type: { stable: {} },
                    is_disabled: false,
                }
            ]
        };

        let result = await this.client.instantiate(this.signer, this.codeId, msg, "instantiate_factory", DEFAULT_FEE);
        this.contractAddress = result.contractAddress;
        return result;
    }

    async getConfig(): Promise<any> {
        let msg = {
            "config": {}
        }

        let result = await this.client.queryContractSmart(this.contractAddress!, msg);
        return result;
    }

    async createPair(assetA: TokenAsset | NativeAsset, assetB: TokenAsset | NativeAsset, amp: number) {
        let msg = {
            create_pair: {
                asset_infos: [
                    assetA.getInfo(),
                    assetB.getInfo(),
                ],
                pair_type: { stable: {} },
                init_params: toEncodedBinary({ amp: amp })
            }
        }

        let createPairRes = await this.client.execute(this.signer, this.contractAddress!, msg, DEFAULT_FEE);

        console.log("print events from create_pair message");
        for(let el of createPairRes.logs[0].events) {
            console.log();
            console.log(el.type);
            for(let attr of el.attributes) {
                console.log(`${attr.key} = ${attr.value}`);
            }
        }
    }
}

export class Pair {
    client: SigningCosmWasmClient
    signer: string
    codeId: number
    contractAddress?: string

    constructor(client: SigningCosmWasmClient, signer: string, codeId: number, contractAddress?: string) {
        this.client = client;
        this.codeId = codeId;
        this.signer = signer;
        if(contractAddress) this.contractAddress = contractAddress;
    }

    async swapNative(offer_asset: NativeAsset): Promise<ExecuteResult> {
        let msg = {
            swap: {
                offer_asset: offer_asset.withAmount(),
                to: this.signer,
            }
        }

        return await this.client.execute(this.signer, this.contractAddress!, msg, DEFAULT_FEE, "swap_native", [offer_asset.toCoin()]);
    }

    async swapToken(offer_asset: TokenAsset): Promise<ExecuteResult> {
        let msg = {
            swap: {
                offer_asset: offer_asset.withAmount()
            }
        }

        let token = new Token(this.client, this.signer, TOKEN_CODE_ID, offer_asset.addr);
        return await token.send(this.contractAddress!, offer_asset.amount!, msg)
    }

    async provideLiquidity(assetA: TokenAsset | NativeAsset, assetB: TokenAsset | NativeAsset): Promise<ExecuteResult> {
        let msg = {
            "provide_liquidity": {
                "assets": [
                    assetA.withAmount(),
                    assetB.withAmount(),
                ]
            }
        }

        let funds = [];
        if (assetA instanceof NativeAsset) {
            funds.push(assetA.toCoin());
        }

        if (assetB instanceof NativeAsset) {
            funds.push(assetB.toCoin());
        }

        return await this.client.execute(this.signer, this.contractAddress!, msg, DEFAULT_FEE, "provide_liquidity", funds);
    }

    async withdrawLiquidity(lp_addr: string, amount: string): Promise<ExecuteResult> {
        let hookMsg = Buffer.from(JSON.stringify({withdraw_liquidity: {}})).toString("base64");
        let msg = {
            send: {
                contract: this.contractAddress!,
                amount: amount,
                msg: hookMsg,
            }
        }

        return await this.client.execute(this.signer, lp_addr, msg, DEFAULT_FEE);
    }

    async queryPairInfo(): Promise<any> {
        return await this.client.queryContractSmart(this.contractAddress!, { pair: {} })
    }

    async debugSwapLog(tx: ExecuteResult) {
        console.log("[swap transaction logs]");
        let keys = ["offer_amount", "return_amount", "commission_amount", "spread_amount"]
        
        for(let el of tx.logs[0].events) {
            for(let attr of el.attributes) {
                if(keys.indexOf(attr.key) != -1) console.log(`${attr.key} = ${attr.value}`);
            }
        }
    }
}

export class Token {
    client: SigningCosmWasmClient
    signer: string
    codeId: number
    contractAddress?: string

    constructor(client: SigningCosmWasmClient, signer: string, codeId: number, contractAddress?: string) {
        this.client = client;
        this.codeId = codeId;
        this.signer = signer;
        if(contractAddress) this.contractAddress = contractAddress;
    }

    async initialize(addr: string, name: string, symbol: string): Promise<InstantiateResult> {
        let msg = {
            name: name,
            symbol: symbol,
            decimals: 6,
            initial_balances: [{
                address: addr,
                amount: String(1_000_000_000_000000)
            }],
            mint: {
                minter: addr
            }
        };

        let result = await this.client.instantiate(this.signer, this.codeId, msg, "init token", DEFAULT_FEE);
        this.contractAddress = result.contractAddress;
        return result;
    }

    async transfer(recipient: string, amount: string): Promise<ExecuteResult> {
        let msg = {
            transfer: { recipient: recipient, amount: amount}
        };

        return await this.client.execute(this.signer, this.contractAddress!, msg, DEFAULT_FEE);
    }

    async send(contract: string, amount: string, message: object): Promise<ExecuteResult> {
        let msg = {
            send: { contract: contract, amount: amount, msg: toEncodedBinary(message)}
        };

        return await this.client.execute(this.signer, this.contractAddress!, msg, DEFAULT_FEE);
    }

    async approve(spender: string, amount: string): Promise<ExecuteResult> {
        let msg = {
            increase_allowance: {
                spender: spender,
                amount: amount,
            },
        }

        return await this.client.execute(this.signer, this.contractAddress!, msg, DEFAULT_FEE);
    }

    async balanceOf(address: string): Promise<any> {
        let msg = {
            balance: { address }
        };

        return await this.client.queryContractSmart(this.contractAddress!, msg)
    }

    async allowance(owner: string, spender: string): Promise<any> {
        let msg = {
            allowance: { owner, spender }
        };

        return await this.client.queryContractSmart(this.contractAddress!, msg)
    }
}