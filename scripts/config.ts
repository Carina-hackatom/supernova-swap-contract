import { calculateFee, GasPrice } from "@cosmjs/stargate"

// default setting | this variables must be configured before executing `npm run` command;
export const MNEMONIC            = "lonely add pledge thing copper crater family grow talent icon basket garage endless sister inquiry tube refuse fresh minor kind pudding regret item install";
export const ALICE_MNEMONIC      = "aunt enough replace gown slim behind connect trash fever achieve ladder assist merge cross actor female laptop guard brave endorse excite fossil dry scout";
export const BOB_MNEMONIC        = "jacket shy initial arm blade object pipe filter prevent absent quote child spatial laundry local cup enjoy source dish near bachelor join romance eager";
export const RPC_ENDPOINT        = "http://k8s-supernov-novanode-7125584468-1b8726bf3e5963e7.elb.ap-northeast-2.amazonaws.com:26657/";
export const BASE_DENOM          = "uatom";

export const WASM_TOKEN          = "../artifacts/novaswap_token-aarch64.wasm";
export const WASM_FACTORY        = "../artifacts/novaswap_factory-aarch64.wasm";
export const WASM_PAIR_STABLE    = "../artifacts/novaswap_pair-aarch64.wasm";

export const DEFAULT_GAS_PRICE   = GasPrice.fromString(`0.01${BASE_DENOM}`);
export const UPLOAD_FEE          = calculateFee(50000000, DEFAULT_GAS_PRICE);
export const DEFAULT_FEE         = calculateFee(10000000, DEFAULT_GAS_PRICE);

// after executing `npm run upload`, you should change this values
export const TOKEN_CODE_ID       = 1;
export const PAIR_CODE_ID        = 3;
export const FACTORY_CODE_ID     = 2;

// after executing `npm run test_cw20`, you should change this values
export const TOKEN_A_CONTRACT    = "cosmos1kwdranvwf6vl2grh99layugwdnph6um2x0c0l8qyvrcgsjcykuhsf494xe";
export const TOKEN_B_CONTRACT    = "cosmos12njsx22ne73swjqxxn5e7xtc2n95y2aw8r73cqdth0g86way24cqnqgn0x";

// after executing `npm run create_pair`, you should change this values
export const FACTORY_CONTRACT    = "cosmos1hpgq5juh354nepq5wmwyddts3eex9t02rd4zrhqqv5nsrpht9r6slmrtgu";
export const POOL_CONTRACT       = "cosmos18vq6emxwq0s77wpt0f5e4zujdjfndcs0kqlr7u8nn2uwv03nef8qg2z76t";
export const LP_TOKEN_ADDR       = "cosmos19jq6mj84cnt9p7sagjxqf8hxtczwc8wlpuwe4sh62w45aheseueszehj9h";