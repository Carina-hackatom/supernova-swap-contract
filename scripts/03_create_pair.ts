import * as config from "./config.js";

import { NativeAsset, newAliceClient, newSenderClient, toEncodedBinary, TokenAsset, upload } from "./helpers.js";
import { Factory, Pair } from "./lib/library.js";

async function run() {
    try {
        let [client, address] = await newSenderClient();

        let factory = new Factory(client, address, config.FACTORY_CODE_ID);

        let tx = await factory.initialize(address, config.TOKEN_CODE_ID, config.PAIR_CODE_ID);
        console.log(`factory instantiated at ${tx.contractAddress}`);
        console.log();

        let amp = 50;
        await factory.createPair(
            new TokenAsset(config.TOKEN_A_CONTRACT),
            new NativeAsset(config.BASE_DENOM),
            amp,
        );
    } catch(err) {
        console.error("Failed to run create pair scripts.");
        console.error(err);
    }
}
run();