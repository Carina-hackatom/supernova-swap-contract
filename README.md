# Supernova Core Contracts
The Ultimate Money Lego for Cosmos Ecosystem

### Environment Setup
```shell
$ rustup default stable 
$ rustup target add wasm32-unknown-unknown // install wasm
$ cargo install cargo-run-script // run scripts defined `in .cargo/config` file.
```

### Build
```shell
$ cd ./contracts/<contract_name>
$ RUSTFLAGS='-C link-arg=-s' cargo wasm
```

### Minimize wasm file.
```shell
$ docker run --rm -v "$(pwd)":/code \
  --mount type=volume,source="$(basename "$(pwd)")_cache",target=/code/target \
  --mount type=volume,source=registry_cache,target=/usr/local/cargo/registry \
  cosmwasm/workspace-optimizer:0.12.3
```

### Run unit test
```shell
$ cargo test
```