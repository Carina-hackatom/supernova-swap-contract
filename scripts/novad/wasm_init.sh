#!/bin/bash

# -- common options --
CHAIN_ID="testing"
NODE="tcp://k8s-supernov-novanode-7125584468-1b8726bf3e5963e7.elb.ap-northeast-2.amazonaws.com:26657/"
WALLET_NAME="wallet"
WALLET=$(novad keys show -a wallet --keyring-backend test)
GASOPTION="--gas 9000000 --gas-prices 0.025uatom"

# -- custom options --
CODE_ID=""

echo "Initiate cosmwasm contract."
echo ""
INIT='{}' # json
novad tx wasm instantiate $CODE_ID "$INIT" \
    --from $WALLET_NAME \
    --label "test label 1" \
    --chain-id $CHAIN_ID \
    --node $NODE \
    $GASOPTION -y