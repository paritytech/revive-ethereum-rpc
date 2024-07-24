.PHONY: node
node:
	RUST_LOG="error,evm=trace,sc_rpc_server=info,runtime::contracts=trace" cargo run -p node -- --dev --consensus instant-seal

.PHONY: rpc
rpc:
	cargo run -p eth-rpc --features dev

.PHONY: demo
demo:
	cd demo && npm run dev

.PHONY: metadata
metadata:
	@subxt explore  --url ws://localhost:9944 pallet ContractsEvm constants chainId > /dev/null
	subxt metadata  --url ws://localhost:9944 -o eth-rpc-server/metadata.scale
	touch eth-rpc-server/src/subxt_client.rs

SALT ?= 0
deploy_dummy: 
	RUST_LOG=info RUST_BACKTRACE=1 cargo run --manifest-path eth-rpc-server/Cargo.toml --example deploy-dummy-contract $(SALT)

codegen: 
	cargo run -p eth-rpc-api_codegen
	cargo +nightly fmt
