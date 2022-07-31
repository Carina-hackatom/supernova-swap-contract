#!/bin/bash

set -eo pipefail

CHAIN_ID="testing"
NODE="tcp://k8s-supernov-novanode-7125584468-1b8726bf3e5963e7.elb.ap-northeast-2.amazonaws.com:26657/"
WALLET=$(novad keys show -a wallet --keyring-backend test)
GASOPTION="--gas 9000000 --gas-prices 0.025uatom"
RECIPIENT="cosmos1lsagfzrm4gz28he4wunt63sts5xzmczw8pkek3"

echo "balances in $WALLET"
novad query bank balances $WALLET --chain-id $CHAIN_ID --node $NODE

echo ""
echo "sending 1000000 uatom"
novad tx bank send $WALLET $RECIPIENT "100000000uatom" --chain-id $CHAIN_ID --node $NODE --keyring-backend test