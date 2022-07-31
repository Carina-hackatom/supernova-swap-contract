import { SigningCosmWasmClient, SigningCosmWasmClientOptions } from "@cosmjs/cosmwasm-stargate";
import { Coin, coin, DirectSecp256k1HdWallet } from "@cosmjs/proto-signing";
import { StdFee } from "@cosmjs/stargate";
import { readFileSync } from "fs";
import * as config from "./config.js"

export async function newAliceClient(): Promise<[SigningCosmWasmClient, string]> {
    return newClient(config.ALICE_MNEMONIC);
}

export async function newBobClient(): Promise<[SigningCosmWasmClient, string]> {
    return newClient(config.ALICE_MNEMONIC);
}

export async function newSenderClient(): Promise<[SigningCosmWasmClient, string]> {
    return newClient(config.MNEMONIC);
}

export async function newClient(mnemonic: string): Promise<[SigningCosmWasmClient, string]> {
    const wallet = await DirectSecp256k1HdWallet.fromMnemonic(mnemonic);
    const [firstAccount] = await wallet.getAccounts();
    let address = firstAccount.address;

    let options: SigningCosmWasmClientOptions = {
        broadcastTimeoutMs: 1000000,
    }
    const client = await SigningCosmWasmClient.connectWithSigner(config.RPC_ENDPOINT, wallet, options);
    return [client, address];
}

export function toEncodedBinary(object: any) {
    return Buffer.from(JSON.stringify(object)).toString('base64');
}

export async function upload(
    client: SigningCosmWasmClient, 
    senderAddress: string, 
    wasmCode: string, 
    fee: StdFee): Promise<number> {
    let contract = await readFileSync(wasmCode);
    let result = await client.upload(senderAddress, contract, fee);
    let codeId = result.codeId;
    return codeId
}

export class NativeAsset {
    denom: string;
    amount?: string

    constructor(denom: string, amount?: string) {
        this.denom = denom
        this.amount = amount
    }

    getInfo() {
        return {
            "native_token": {
                "denom": this.denom,
            }
        }
    }

    withAmount() {
        return {
            "info": this.getInfo(),
            "amount": this.amount
        }
    }

    getDenom() {
        return this.denom
    }

    toCoin():Coin {
        return coin(this.amount || "0", this.denom)
    }
}

export class TokenAsset {
    addr: string;
    amount?: string

    constructor(addr: string, amount?: string) {
        this.addr = addr
        this.amount = amount
    }

    getInfo() {
        return {
            "token": {
                "contract_addr": this.addr
            }
        }
    }

    withAmount() {
        return {
            "info": this.getInfo(),
            "amount": this.amount
        }
    }

    toCoin() {
        return null
    }

    getDenom() {
        return this.addr
    }
}