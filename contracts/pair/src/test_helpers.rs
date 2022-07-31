use std::{fmt::Display, ops::Mul, str::FromStr};

use cosmwasm_std::{
    testing::{mock_env, mock_info},
    to_binary, BlockInfo, Decimal, DepsMut, Env, Reply, SubMsgResponse, SubMsgResult, Timestamp,
    Uint128,
};
use cw20::Cw20ReceiveMsg;
use novaswap::pair::{Cw20HookMsg, ExecuteMsg};
use prost::Message;

use crate::contract::{execute, reply};

pub const TOKEN_DECIMALS: u128 = 1_000000000000000000u128;

pub const TEST_SWAP_DECIMALS: u128 = 1_00000000000u128;

#[derive(Clone, PartialEq, Message)]
struct MsgInstantiateContractResponse {
    #[prost(string, tag = "1")]
    pub contract_address: ::prost::alloc::string::String,
    #[prost(bytes, tag = "2")]
    pub data: ::prost::alloc::vec::Vec<u8>,
}

pub fn store_liquidity_token(deps: DepsMut, msg_id: u64, contract_addr: String) {
    let data = MsgInstantiateContractResponse {
        contract_address: contract_addr,
        data: vec![],
    };

    let mut encoded_instantiate_reply = Vec::<u8>::with_capacity(data.encoded_len());
    data.encode(&mut encoded_instantiate_reply).unwrap();

    let reply_msg = Reply {
        id: msg_id,
        result: SubMsgResult::Ok(SubMsgResponse {
            events: vec![],
            data: Some(encoded_instantiate_reply.into()),
        }),
    };

    reply(deps, mock_env(), reply_msg).unwrap();
}

pub fn mock_env_with_block_time(time: u64) -> Env {
    let mut env = mock_env();
    env.block = BlockInfo {
        height: 1,
        time: Timestamp::from_seconds(time),
        chain_id: "columbus".to_string(),
    };
    env
}

pub fn swap_token(deps: DepsMut, swap_amount: Uint128) -> SwapResult {
    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: String::from("addr0000"),
        amount: swap_amount.mul(Uint128::from(TEST_SWAP_DECIMALS)),
        msg: to_binary(&Cw20HookMsg::Swap {
            belief_price: None,
            max_spread: Some(Decimal::from_str("0.4").unwrap()),
            to: None,
        })
        .unwrap(),
    });
    let env = mock_env_with_block_time(1000);
    let info = mock_info("asset0000", &[]);

    let res = execute(deps, env, info, msg).unwrap();
    let offer_amount = &res.attributes.get(5).expect("no message").value;
    let return_amount = &res.attributes.get(6).expect("no message").value;
    let spread_amount = &res.attributes.get(7).expect("no message").value;
    let commission_amount = &res.attributes.get(8).expect("no message").value;

    SwapResult {
        offer_amount: Uint128::try_from(offer_amount.as_str()).unwrap(),
        return_amount: Uint128::try_from(return_amount.as_str()).unwrap(),
        spread_amount: Uint128::try_from(spread_amount.as_str()).unwrap(),
        commission_amount: Uint128::try_from(commission_amount.as_str()).unwrap(),
    }
}

pub struct SwapResult {
    pub offer_amount: Uint128,
    pub return_amount: Uint128,
    pub spread_amount: Uint128,
    pub commission_amount: Uint128,
}

impl Display for SwapResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let actual_amount = self
            .return_amount
            .checked_add(self.commission_amount)
            .unwrap();
        return write!(
            f,
            "offer: {}, return: {}, spread: {}, commission: {}, actual: {}",
            self.offer_amount,
            self.return_amount,
            self.spread_amount,
            self.commission_amount,
            actual_amount
        );
    }
}

// swap simulation datas
pub struct SwapTestCase {
    pub token_a: u128,       // token amount of A in liquidity before swap operation
    pub token_b: u128,       // token amount of B in liquidity before swap operation
    pub swap_amount: u128,   // swap amount
    pub after_token_a: u128, // token amount of A in liquidity after swap operation
    pub after_token_b: u128, // token amount of B in liquidity after swap operation
}

pub const CASES: [SwapTestCase; 51] = [
    SwapTestCase {
        token_a: 50_0000000u128,
        token_b: 150_0000000u128,
        swap_amount: 2_0000000u128,
        after_token_a: 52_0000000u128,
        after_token_b: 148_2967759u128,
    },
    SwapTestCase {
        token_a: 52_0000000u128,
        token_b: 148_2967759u128,
        swap_amount: 2_0000000u128,
        after_token_a: 54_0000000u128,
        after_token_b: 146_2660842u128,
    },
    SwapTestCase {
        token_a: 54_0000000u128,
        token_b: 146_2660842u128,
        swap_amount: 2_0000000u128,
        after_token_a: 56_0000000u128,
        after_token_b: 144_2380329u128,
    },
    SwapTestCase {
        token_a: 56_0000000u128,
        token_b: 144_2380329u128,
        swap_amount: 2_0000000u128,
        after_token_a: 58_0000000u128,
        after_token_b: 142_2123685u128,
    },
    SwapTestCase {
        token_a: 58_0000000u128,
        token_b: 142_2123685u128,
        swap_amount: 2_0000000u128,
        after_token_a: 60_0000000u128,
        after_token_b: 140_1888723u128,
    },
    SwapTestCase {
        token_a: 60_0000000u128,
        token_b: 140_1888723u128,
        swap_amount: 2_0000000u128,
        after_token_a: 62_0000000u128,
        after_token_b: 138_1673548u128,
    },
    SwapTestCase {
        token_a: 62_0000000u128,
        token_b: 138_1673548u128,
        swap_amount: 2_0000000u128,
        after_token_a: 64_0000000u128,
        after_token_b: 136_1476511u128,
    },
    SwapTestCase {
        token_a: 64_0000000u128,
        token_b: 136_1476511u128,
        swap_amount: 2_0000000u128,
        after_token_a: 66_0000000u128,
        after_token_b: 134_1296174u128,
    },
    SwapTestCase {
        token_a: 66_0000000u128,
        token_b: 134_1296174u128,
        swap_amount: 2_0000000u128,
        after_token_a: 68_0000000u128,
        after_token_b: 132_113128u128,
    },
    SwapTestCase {
        token_a: 68_0000000u128,
        token_b: 132_1131280u128,
        swap_amount: 2_0000000u128,
        after_token_a: 70_0000000u128,
        after_token_b: 130_0980727u128,
    },
    SwapTestCase {
        token_a: 70_0000000u128,
        token_b: 130_0980727u128,
        swap_amount: 2_0000000u128,
        after_token_a: 72_0000000u128,
        after_token_b: 128_0843548u128,
    },
    SwapTestCase {
        token_a: 72_0000000u128,
        token_b: 128_0843548u128,
        swap_amount: 2_0000000u128,
        after_token_a: 74_0000000u128,
        after_token_b: 126_0718895u128,
    },
    SwapTestCase {
        token_a: 74_0000000u128,
        token_b: 126_0718895u128,
        swap_amount: 2_0000000u128,
        after_token_a: 76_0000000u128,
        after_token_b: 124_0606022u128,
    },
    SwapTestCase {
        token_a: 76_0000000u128,
        token_b: 124_0606022u128,
        swap_amount: 2_0000000u128,
        after_token_a: 78_0000000u128,
        after_token_b: 122_0504275u128,
    },
    SwapTestCase {
        token_a: 78_0000000u128,
        token_b: 122_0504275u128,
        swap_amount: 2_0000000u128,
        after_token_a: 80_0000000u128,
        after_token_b: 120_0413082u128,
    },
    SwapTestCase {
        token_a: 80_0000000u128,
        token_b: 120_0413082u128,
        swap_amount: 2_0000000u128,
        after_token_a: 82_0000000u128,
        after_token_b: 118_0331943u128,
    },
    SwapTestCase {
        token_a: 82_0000000u128,
        token_b: 118_0331943u128,
        swap_amount: 2_0000000u128,
        after_token_a: 84_0000000u128,
        after_token_b: 116_0260422u128,
    },
    SwapTestCase {
        token_a: 84_0000000u128,
        token_b: 116_0260422u128,
        swap_amount: 2_0000000u128,
        after_token_a: 86_0000000u128,
        after_token_b: 114_0198146u128,
    },
    SwapTestCase {
        token_a: 86_0000000u128,
        token_b: 114_0198146u128,
        swap_amount: 2_0000000u128,
        after_token_a: 88_0000000u128,
        after_token_b: 112_0144792u128,
    },
    SwapTestCase {
        token_a: 88_0000000u128,
        token_b: 112_0144792u128,
        swap_amount: 2_0000000u128,
        after_token_a: 90_0000000u128,
        after_token_b: 110_0100091u128,
    },
    SwapTestCase {
        token_a: 90_0000000u128,
        token_b: 110_0100091u128,
        swap_amount: 2_0000000u128,
        after_token_a: 92_0000000u128,
        after_token_b: 108_0063818u128,
    },
    SwapTestCase {
        token_a: 92_0000000u128,
        token_b: 108_0063818u128,
        swap_amount: 2_0000000u128,
        after_token_a: 94_0000000u128,
        after_token_b: 106_0035791u128,
    },
    SwapTestCase {
        token_a: 94_0000000u128,
        token_b: 106_0035791u128,
        swap_amount: 2_0000000u128,
        after_token_a: 96_0000000u128,
        after_token_b: 104_0015873u128,
    },
    SwapTestCase {
        token_a: 96_0000000u128,
        token_b: 104_0015873u128,
        swap_amount: 2_0000000u128,
        after_token_a: 98_0000000u128,
        after_token_b: 102_0003963u128,
    },
    SwapTestCase {
        token_a: 98_0000000u128,
        token_b: 102_0003963u128,
        swap_amount: 2_0000000u128,
        after_token_a: 100_0000000u128,
        after_token_b: 100_0000000u128,
    },
    SwapTestCase {
        token_a: 100_0000000u128,
        token_b: 100_0000000u128,
        swap_amount: 2_0000000u128,
        after_token_a: 102_0000000u128,
        after_token_b: 98_0003961u128,
    },
    SwapTestCase {
        token_a: 102_0000000u128,
        token_b: 98_0003961u128,
        swap_amount: 2_0000000u128,
        after_token_a: 104_0000000u128,
        after_token_b: 96_0015860u128,
    },
    SwapTestCase {
        token_a: 104_0000000u128,
        token_b: 96_0015860u128,
        swap_amount: 2_0000000u128,
        after_token_a: 106_0000000u128,
        after_token_b: 94_0035748u128,
    },
    SwapTestCase {
        token_a: 106_0000000u128,
        token_b: 94_0035748u128,
        swap_amount: 2_0000000u128,
        after_token_a: 108_0000000u128,
        after_token_b: 92_0063715u128,
    },
    SwapTestCase {
        token_a: 108_0000000u128,
        token_b: 92_0063715u128,
        swap_amount: 2_0000000u128,
        after_token_a: 110_0000000u128,
        after_token_b: 90_0099889u128,
    },
    SwapTestCase {
        token_a: 110_0000000u128,
        token_b: 90_0099889u128,
        swap_amount: 2_0000000u128,
        after_token_a: 112_0000000u128,
        after_token_b: 88_0144438u128,
    },
    SwapTestCase {
        token_a: 112_0000000u128,
        token_b: 88_0144438u128,
        swap_amount: 2_0000000u128,
        after_token_a: 114_0000000u128,
        after_token_b: 86_0197575u128,
    },
    SwapTestCase {
        token_a: 114_0000000u128,
        token_b: 86_0197575u128,
        swap_amount: 2_0000000u128,
        after_token_a: 116_0000000u128,
        after_token_b: 84_0259555u128,
    },
    SwapTestCase {
        token_a: 116_0000000u128,
        token_b: 84_0259555u128,
        swap_amount: 2_0000000u128,
        after_token_a: 118_0000000u128,
        after_token_b: 82_033068u128,
    },
    SwapTestCase {
        token_a: 118_0000000u128,
        token_b: 82_0330683u128,
        swap_amount: 2_0000000u128,
        after_token_a: 120_0000000u128,
        after_token_b: 80_0411313u128,
    },
    SwapTestCase {
        token_a: 120_0000000u128,
        token_b: 80_0411313u128,
        swap_amount: 2_0000000u128,
        after_token_a: 122_0000000u128,
        after_token_b: 78_0501860u128,
    },
    SwapTestCase {
        token_a: 122_0000000u128,
        token_b: 78_0501860u128,
        swap_amount: 2_0000000u128,
        after_token_a: 124_0000000u128,
        after_token_b: 76_0602795u128,
    },
    SwapTestCase {
        token_a: 124_0000000u128,
        token_b: 76_0602795u128,
        swap_amount: 2_0000000u128,
        after_token_a: 126_0000000u128,
        after_token_b: 74_0714662u128,
    },
    SwapTestCase {
        token_a: 126_0000000u128,
        token_b: 74_0714662u128,
        swap_amount: 2_0000000u128,
        after_token_a: 128_0000000u128,
        after_token_b: 72_0838078u128,
    },
    SwapTestCase {
        token_a: 128_0000000u128,
        token_b: 72_0838078u128,
        swap_amount: 2_0000000u128,
        after_token_a: 130_0000000u128,
        after_token_b: 70_0973745u128,
    },
    SwapTestCase {
        token_a: 130_0000000u128,
        token_b: 70_0973745u128,
        swap_amount: 2_0000000u128,
        after_token_a: 132_0000000u128,
        after_token_b: 68_1122460u128,
    },
    SwapTestCase {
        token_a: 132_0000000u128,
        token_b: 68_1122460u128,
        swap_amount: 2_0000000u128,
        after_token_a: 134_0000000u128,
        after_token_b: 66_1285126u128,
    },
    SwapTestCase {
        token_a: 134_0000000u128,
        token_b: 66_1285126u128,
        swap_amount: 2_0000000u128,
        after_token_a: 136_0000000u128,
        after_token_b: 64_1462771u128,
    },
    SwapTestCase {
        token_a: 136_0000000u128,
        token_b: 64_1462771u128,
        swap_amount: 2_0000000u128,
        after_token_a: 138_0000000u128,
        after_token_b: 62_1656559u128,
    },
    SwapTestCase {
        token_a: 138_0000000u128,
        token_b: 62_1656559u128,
        swap_amount: 2_0000000u128,
        after_token_a: 140_0000000u128,
        after_token_b: 60_1867817u128,
    },
    SwapTestCase {
        token_a: 140_0000000u128,
        token_b: 60_1867817u128,
        swap_amount: 2_0000000u128,
        after_token_a: 142_0000000u128,
        after_token_b: 58_2098053u128,
    },
    SwapTestCase {
        token_a: 142_0000000u128,
        token_b: 58_2098053u128,
        swap_amount: 2_0000000u128,
        after_token_a: 144_0000000u128,
        after_token_b: 56_2348994u128,
    },
    SwapTestCase {
        token_a: 144_0000000u128,
        token_b: 56_2348994u128,
        swap_amount: 2_0000000u128,
        after_token_a: 146_0000000u128,
        after_token_b: 54_2622612u128,
    },
    SwapTestCase {
        token_a: 146_0000000u128,
        token_b: 54_2622612u128,
        swap_amount: 2_0000000u128,
        after_token_a: 148_0000000u128,
        after_token_b: 52_2921176u128,
    },
    SwapTestCase {
        token_a: 148_0000000u128,
        token_b: 52_2921176u128,
        swap_amount: 2_0000000u128,
        after_token_a: 150_0000000u128,
        after_token_b: 50_3247297u128,
    },
    SwapTestCase {
        token_a: 150_0000000u128,
        token_b: 50_3247297u128,
        swap_amount: 2_0000000u128,
        after_token_a: 152_0000000u128,
        after_token_b: 48_3603997u128,
    },
];
