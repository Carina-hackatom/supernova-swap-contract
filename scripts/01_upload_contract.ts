import * as config from "./config.js";

import { newSenderClient, upload } from "./helpers.js";

async function run() {
    try {
        let [client, address] = await newSenderClient();

        console.log(`uploading cosmwasm contracts to: ${config.RPC_ENDPOINT}`);
        console.log(`uploader address: ${address}`);

        let tokenCodeId = await upload(client, address, config.WASM_TOKEN, config.UPLOAD_FEE);
        let pairCodeId = await upload(client, address, config.WASM_PAIR_STABLE, config.UPLOAD_FEE);
        let factoryCodeId = await upload(client, address, config.WASM_FACTORY, config.UPLOAD_FEE);

        console.log(`uploaded token code id: ${tokenCodeId}`);
        console.log(`uploaded pair code id: ${pairCodeId}`);
        console.log(`uploaded factory code id: ${factoryCodeId}`);
    } catch (err) {
        console.error("failed to upload wasm file");
        console.error(err);
    }
}
run();