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
.PHONY: example_deploy
example_deploy: 
	RUST_LOG=info cargo run --manifest-path rpc/Cargo.toml --features example --example deploy $(SALT)

.PHONY: example_transfer
example_transfer: 
	RUST_LOG=info cargo run --manifest-path rpc/Cargo.toml --features example --example transfer

codegen: 
	cargo run -p eth-rpc-codegen
	cargo +nightly fmt
