import * as config from "./config.js";

import { newAliceClient, newSenderClient } from "./helpers.js";
import { Token } from "./lib/library.js";

async function run() {
    try {

        let [client, address] = await newSenderClient();
        let [alice, aliceAddr] = await newAliceClient();

        let tokenA = new Token(client, address, config.TOKEN_CODE_ID);
        let result = await tokenA.initialize(address, "SUPER", "SUPER");
        let tokenA_contract = result.contractAddress;

        let tokenB = new Token(client, address, config.TOKEN_CODE_ID);
        result = await tokenB.initialize(address, "NOVA", "NOVA");
        let tokenB_contract = result.contractAddress;
        
        console.log(`token A contact addr : ${tokenA_contract}`);
        console.log(`token B contact addr : ${tokenB_contract}`);

        let balanceA = await tokenA.balanceOf(address);
        let balanceB = await tokenB.balanceOf(address);

        console.log("token A balance of sender:");
        console.log(balanceA);
        console.log("token B balance of sender:");
        console.log(balanceB);

        let send_amount = String(1_000_000000);
        let tx = await tokenA.transfer(aliceAddr, send_amount);
        console.log(`token A transferred to ${aliceAddr}, amount: ${send_amount}`);

        balanceA = await tokenA.balanceOf(address);
        balanceB = await tokenB.balanceOf(address);

        console.log("token A balance of sender:");
        console.log(balanceA);
        console.log("token B balance of sender:");
        console.log(balanceB);
    } catch (err) {
        console.error("failed to upload wasm file");
        console.error(err);
    }
}
run();