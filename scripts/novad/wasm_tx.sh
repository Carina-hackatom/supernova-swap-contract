# execute fails if wrong person
APPROVE='{"approve":{"quantity":[{"amount":"50000","denom":"usponge"}]}}'
wasmd tx wasm execute $CONTRACT "$APPROVE" --from thief $TXFLAG -y

# looking at the logs should show: "execute wasm contract failed: Unauthorized"
# and bob should still be broke (and broken showing the account does not exist Error)
wasmd query bank balances $(wasmd keys show bob -a) $NODE

# but succeeds when fred tries
wasmd tx wasm execute $CONTRACT "$APPROVE" --from fred $TXFLAG -y