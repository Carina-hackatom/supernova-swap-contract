use cosmwasm_std::{to_binary, Addr, Deps, QueryRequest, StdResult, WasmQuery};

use novaswap::{pair::QueryMsg, pairinfo::PairInfo};

/// Send `QueryMsg::Pair {}` message to get the [`PairInfo`] from pair contract
pub fn query_pair_info(deps: Deps, pair_contract: &Addr) -> StdResult<PairInfo> {
    deps.querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
        contract_addr: pair_contract.to_string(),
        msg: to_binary(&QueryMsg::Pair {})?,
    }))
}
