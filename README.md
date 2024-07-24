# 🚧⚠️ [WIP] pallet-revive Eth RPC Server

The goal of this project is to provide an [Ethereum JSON-RPC](https://ethereum.org/en/developers/docs/apis/json-rpc/) API server for the upcoming RISC-V contracts runtime pallet. (forked off from [pallet-contracts](https://github.com/paritytech/polkadot-sdk/tree/master/substrate/frame/contracts)).

This serves the following purposes:

- Users can interact with RISC-V contracts using an Ethereum compatible wallet.
- Developers can use existing Ethereum tooling to interact with pallet-revive.and deploy contracts wherever the new RISC-V runtime pallet is deployed (e.g AssetHub).

# Project Structure

- `./rpc`
  An Ethereum JSON-RPC server, based on [jsonrpsee](https://github.com/paritytech/jsonrpsee).
  The API is generated from the official Ethereum JSON-RPC [specification](https://github.com/ethereum/execution-apis).
  It uses [subxt](https://github.com/paritytech/subxt) to interact with the Substrate node hosting the RISC-V contracts runtime pallet.

- `./chain`
  A Node bootstrapped from the [minimal template](https://github.com/paritytech/polkadot-sdk/tree/master/templates/minimal)
  The node is configured with pallet-contracts and an [evm adapter pallet](./chain/pallet-contracts-evm).

- `./demo`
  A barebone html page to interact with the node using MetaMask and [Ether.js](https://github.com/ethers-io/ethers.js).

# Getting started

## Start the node

This command starts the node in `--dev` mode.

```bash
make node
```

## Start the RPC server

This command starts the rpc proxy, by default it runs on `localhost:9090`

```bash
make rpc
```

## Run the demo

```bash
make demo
```

This will serve a barebone HTML page at `http://localhost:3000`, that let you interact with the node through [MetaMask](https://metamask.io) using [Ether.js](https://github.com/ethers-io/ethers.js).

### Configure MetaMask

You can use the following instructions to setup [MetaMask](https://metamask.io) with the local chain.

> Note: When you interact with MetaMask and restart the chain, you need to clear the activity tab (Settings > Advanced > Clear activity tab data)
> See [here](https://support.metamask.io/managing-my-wallet/resetting-deleting-and-restoring/how-to-clear-your-account-activity-reset-account) for more info on how to reset the account activity.

#### Add a new network

To interact with the local chain, you need to add a new network in [MetaMask](https://metamask.io).
See [here](https://support.metamask.io/networks-and-sidechains/managing-networks/how-to-add-a-custom-network-rpc/#adding-a-network-manually) for more info on how to add a custom network.

Make sure the node and the proxy are started, and use the following settings to configure the network (MetaMask > Networks > Add a network manually):
  
- Network name: Revive demo
- RPC URL: <http://localhost:9090>
- Chain ID: [596](https://github.com/paritytech/revive-ethereum-rpc/blob/main/chain/runtime/src/lib.rs?plain=1#L198)
- Currency Symbol: `DEV`

#### Import Dev account

You will need to import the following account that is endowed with some balance at genesis to interact with the chain.
See [here](https://support.metamask.io/managing-my-wallet/accounts-and-addresses/how-to-import-an-account/) for more info on how to import an account.

- Account: `0x75E480dB528101a381Ce68544611C169Ad7EB342`
- Private Key: `a872f6cbd25a0e04a08b1e21098017a9e6194d101d75e13111f71410c59cd57f`

# Justification and Background

- [Refereunda 885](https://polkadot.polkassembly.io/referenda/885) - Should we allow EVM compatible contracts on Asset Hub?
- [Forum post](https://forum.polkadot.network/t/hybrid-system-chains-make-polkadot-permissionless/7089) - Hybrid system chains make Polkadot perimisionless
- [Forum post](https://forum.polkadot.network/t/contracts-update-solidity-on-polkavm/6949) - Contracts update: Solidity on PolkaVM

# Technical references

- [Acala](https://github.com/AcalaNetwork/Acala) and [bodhi.js](https://github.com/AcalaNetwork/bodhi.js), Acala's Ethereum RPC server.
- [Ethink](https://github.com/agryaznov/ethink), an Ethereum & Polkadot RPC compatibility POC.
