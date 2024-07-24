#![cfg(test)]

use eth_rpc_api::*;
use indoc::indoc;

use crate::ReceiptInfo;

#[test]
fn parse_receipt_info_works() {
    let json = indoc! {r###"
        {
            "blockHash": "0xfe4fba10cebaf980f48b9b5582bb299f260a29250687a59e9cd568232a87fe40",
            "blockNumber": "0xb",
            "contractAddress": null,
            "cumulativeGasUsed": "0x57f2",
            "effectiveGasPrice": "0x49708940",
            "from": "0xf39fd6e51aad88f6f4ce6ab8827279cfffb92266",
            "gasUsed": "0x57f2",
            "logs": [
                {
                "address": "0x8a791620dd6260079bf849dc5567adc3f2fdc318",
                "blockHash": "0xfe4fba10cebaf980f48b9b5582bb299f260a29250687a59e9cd568232a87fe40",
                "blockNumber": "0xb",
                "data": "0x000000000000000000000000000000000000000000000000000000000000002a",
                "logIndex": "0x0",
                "removed": false,
                "topics": [
                    "0x24abdb5865df5079dcc5ac590ff6f01d5c16edbc5fab4e195d9febd1114503da"
                ],
                "transactionHash": "0x5bc48671e9d5ae3c9386ef4b9d0c19587ff3600f3df4edf8abc7dcf01c44a785",
                "transactionIndex": "0x0"
                }
            ],
            "logsBloom": "0x00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000020000000000000000000000000000000000000000000002000000000000000000000000000002000000000000000000020008000000000000000000000000000000000000000000000000000000000000000000000040000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000",
            "status": "0x1",
            "to": "0x8a791620dd6260079bf849dc5567adc3f2fdc318",
            "transactionHash": "0x5bc48671e9d5ae3c9386ef4b9d0c19587ff3600f3df4edf8abc7dcf01c44a785",
            "transactionIndex": "0x0",
            "type": "0x2"
        }
        "###};

    let _: ReceiptInfo = serde_json::from_str(json).unwrap();
}

#[test]
fn parse_block_works() {
    let json = indoc! {r###"
       {
            "baseFeePerGas": "0x2dae10cc",
            "blobGasUsed": "0x0",
            "difficulty": "0x0",
            "excessBlobGas": "0x0",
            "extraData": "0x",
            "gasLimit": "0x1c9c380",
            "gasUsed": "0x57f2",
            "hash": "0x5012cb95404ad3f783384686a5cc3a12ddf29aa47c905e9246dfefd95b028650",
            "logsBloom": "0x00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000040000000000000000000000000000000000000000000000020000000000000000000000000000000000000000000002000000000000000000200000000000000000000000000000020000000000000000000000000000000000000000000000040000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000",
            "miner": "0xc014ba5ec014ba5ec014ba5ec014ba5ec014ba5e",
            "mixHash": "0x36e0e2ad178d961006334c968a7f38fc9e049070a61e5b8203219acebd7bfd59",
            "nonce": "0x0000000000000000",
            "number": "0x2",
            "parentBeaconBlockRoot": "0x20ac26c307a8005e8e0c24ae0e18f5fb23fb724064e99ebccb2cb877faaa55ac",
            "parentHash": "0x9145ef7e03324557509389723ebb14df145c495a4091a40ea549aa9dff60073f",
            "receiptsRoot": "0xcd3a42a384fac5336359b7fcc59c47125d5c056f33fde9a6290422bdd82cf251",
            "sha3Uncles": "0x1dcc4de8dec75d7aab85b567b6ccd41ad312451b948a7413f0a142fd40d49347",
            "size": "0x2df",
            "stateRoot": "0x4af675a9e8e66a492b8ed1c53be43ddb6ad7c4683c88655b3b8a177c5eb45c17",
            "timestamp": "0x6655d5c7",
            "totalDifficulty": "0x0",
            "transactions": [{
                "accessList": [],
                "blockHash": "0x5012cb95404ad3f783384686a5cc3a12ddf29aa47c905e9246dfefd95b028650",
                "blockNumber": "0x2",
                "chainId": "0x7a69",
                "from": "0xf39fd6e51aad88f6f4ce6ab8827279cfffb92266",
                "gas": "0x1c9c380",
                "gasPrice": "0x6948dacc",
                "hash": "0x60573721ca1f0c4dd80f11dac04eb6403c6b72bc27ceb9ce93dddb5e333bfc62",
                "input": "0xc6888fa10000000000000000000000000000000000000000000000000000000000000006",
                "maxFeePerGas": "0x96f6eb98",
                "maxPriorityFeePerGas": "0x3b9aca00",
                "nonce": "0x1",
                "r": "0xf4a2ead066214a4d6914a66a1ae990c9888352a9cfc41cc6e1d41f3220d01770",
                "s": "0x19864d8a39b06eba7624167fece36dd8dbcb6a4a8d30eb980117904856a694eb",
                "to": "0x5fbdb2315678afecb367f032d93f642f64180aa3",
                "transactionIndex": "0x0",
                "type": "0x2",
                "v": "0x1",
                "value": "0x0"
            }],
            "transactionsRoot": "0x15636dbd51712d5fc191ffeaed96907374574833564d76de8d7262027b4cb364",
            "uncles": [],
            "withdrawals": [],
            "withdrawalsRoot": "0x56e81f171bcc55a6ff8345e692c0f86e5b48e01b996cadc001622fb5e363b421"
        } 
        "###};

    let block: Block = serde_json::from_str(json).unwrap();
    dbg!(block);
}
