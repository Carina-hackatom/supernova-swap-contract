#!/bin/bash

WASM_DIR="../../artifacts/novaswap_token-aarch64.wasm"

CHAIN_ID="testing"
NODE="tcp://localhost:26657/"
WALLET_NAME="validator1"
HOME_DIR=$HOME/.novad/validator1
GASOPTION="--gas 9000000 --gas-prices 0.025uatom"

# 1. Upload
echo "upload wasm file to the blockchain."
novad tx wasm store $WASM_DIR \
    --from $WALLET_NAME --gas-prices 0.1uatom --gas 1000000000 \
    --gas-adjustment 1.3 -b block -y \
    --chain-id $CHAIN_ID \
    --home $HOME_DIR \
    --keyring-backend test \
    --node $NODE 