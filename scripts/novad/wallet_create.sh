#!/bin/bash

set -eo pipefail

# create wallet
novad keys add wallet --keyring-backend test

# show wallet
novad keys show -a wallet --keyring-backend test