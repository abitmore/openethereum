// Copyright 2020-2021 Gnosis Ltd.
// Copyright 2015-2020 Parity Technologies (UK) Ltd.
// This file is part of OpenEthereum.

// OpenEthereum is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// OpenEthereum is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with OpenEthereum.  If not, see <http://www.gnu.org/licenses/>.

//! Chain Frozen State
//!
//! Provides a way for blockchains to "freeze" a given set of historical
//! transactions by hardcoding the outcome of their executions.
//!
//! When a fronzen transaction is encountered on the blockchain, it will not
//! be executed by the VM against the latest world state, the hardcoded balance
//! and storage changes will be applied insead without touching the actual code.
//!
//! The scenario that brought up the need for this feature was disabling the WASM
//! VM engine. At the time of writing the Kovan Testnet had several hundred transactions
//! that invoked the WASM VM. This feature is used to store the historical results of
//! old known transactinos prior to disabling and removing WASM so that the chain could
//! still function and sync old blocks with the pWASM engine code removed from the codebase.
//!
//! The structure of a single frozen transaction in a given block, with changes
//! to the balances and storage values, returning receipts with logs looks as following:
//!
//! ```json
//! "7103749": [ // block number
//!   {
//!     "id": "b127f7d546309857bcc5d03b4532e641749e196f7cdcb45789b914f989dbc8cd", // transaction id
//!     "balanceOps": [
//!       {
//!         "account": "0x05ba9a1d453ed591f70e5884a5eded482400bb62",
//!         "amount": "0x642fc026aa8000",
//!         "op": "sub"
//!       },
//!       {
//!         "account": "0x05ba9a1d453ed591f70e5884a5eded482400bb62",
//!         "amount": "0x60bd080aa20000",
//!         "op": "add"
//!       }
//!     ],
//!     "storageChanges": [
//!        {
//!           "account": "0xfe3552a8444c54a9b3d79cf890063d345562ea9d",
//!           "key": "0x20000000000000000000000000000000000000000000000000000000000000",
//!           "value": "0x98968"
//!         },
//!         {
//!           "account": "0xfe3552a8444c54a9b3d79cf890063d345562ea9d",
//!           "key": "0x1000000000000000000000005ba9a1d453ed591f70e5884a5eded482400bb6",
//!           "value": "0x98968"
//!         },
//!         {
//!           "account": "0xfe3552a8444c54a9b3d79cf890063d345562ea9d",
//!           "key": "0x30000000000000000000000000000000000000000000000000000000000000",
//!           "value": "0x5ba9a1d453ed591f70e5884a5eded482400bb6"
//!         }
//!       ],
//!     "receipt": {
//!       "gasUsed": "0x7e60",
//!       "logBloom": "dbc9c830d20100", // hex(deflate_bytes(logs_bloom))
//!       "logs": [
//!         {
//!           "address": "0xfe3552a8444c54a9b3d79cf890063d345562ea9d",
//!           "topics": [
//!             "0xddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef",
//!             "0x00000000000000000000000005ba9a1d453ed591f70e5884a5eded482400bb62",
//!             "0x00000000000000000000000000859735759eede1232fb2567254d0e69c89d542"
//!           ],
//!           "data": "0x00000000000000000000000000000000000000000000000000000000000000c8"
//!         }
//!       ],
//!       "outcome": {
//!         "StatusCode": 1
//!       }
//!     }
//!   }
//! ],
//! ```

use ethereum_types::{Address, H256, U256};
use std::{collections::BTreeMap, io::Read};

/// Encapsulates all possible effects a transaction
/// execution may have on the world state.
#[serde(rename_all = "camelCase")]
#[derive(Debug, Deserialize)]
pub struct TransactionTrace {
    /// Globally unique transaction hash
    pub id: H256,

    /// Changes to any balances in accounts caused by a transaction
    /// This also includes transfer of gas fees to the miner.
    pub balance_ops: Vec<BalanceOp>,

    /// All key-value pairs in account state trees that are set
    /// as a result of running this transaction.
    pub storage_changes: Vec<StorageChange>,

    /// All logs/events generated by this transaction.
    pub receipt: common_types::receipt::TypedReceipt,
}

/// defines whether an account balance increases
/// or decreases by a given amount
#[serde(rename_all = "camelCase")]
#[derive(Debug, Eq, PartialEq, Deserialize)]
pub enum Op {
    Add,
    Sub,
}

/// represents a single change in balance
/// for an account, relative to its prior value
/// before running the transaction.
#[serde(rename_all = "camelCase")]
#[derive(Debug, Deserialize)]
pub struct BalanceOp {
    pub account: Address,
    pub amount: U256,
    pub op: Op,
}

/// defines the equivalent of one SSTORE operation
/// that sets one key to a 256 bit value at a given
/// contract state subtree.
#[serde(rename_all = "camelCase")]
#[derive(Debug, Deserialize)]
pub struct StorageChange {
    /// Address of the contract owning the state
    pub account: Address,

    /// a 256 bit key in the contract storage address space.
    pub key: H256,

    /// a 256 bit (32 byte) arbitary value stored under key
    /// in account's state tree.
    pub value: U256,
}

/// Key is the block number, and the value is a list of transactions
/// within that block with the results of their execution. The assumption
/// is that all values are always sorted by the block number and tx chronologically.
type FrozenChainState = BTreeMap<u64, Vec<TransactionTrace>>;

/// Deserializes a serialized frozen state into a map of blocks and transactions.
pub fn restore_frozen_state<R: Read>(read: R) -> Result<FrozenChainState, serde_json::Error> {
    serde_json::from_reader::<R, FrozenChainState>(read)
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use common_types::receipt::{TransactionOutcome, TypedReceipt};

    use super::*;

    #[test]
    fn can_deserialize_frozen_block() {
        let serialized_block = br#"{
            "8519876": [
                {
                  "id": "0x5c09d643b9f6a7cf9065e1ee2f47223ff74592428cb1181b4286145d4925504c",
                  "balanceOps": [
                    {
                      "account": "0x1f256d9fd1fbb4b514784584557751b0e2f81185",
                      "amount": "0x2b6a067727c000",
                      "op": "sub"
                    },
                    {
                      "account": "0x1f256d9fd1fbb4b514784584557751b0e2f81185",
                      "amount": "0x28983165f80a00",
                      "op": "add"
                    },
                    {
                      "account": "0x0010f94b296a852aaac52ea6c5ac72e03afd032d",
                      "amount": "0x2d1d5112fb600",
                      "op": "add"
                    }
                  ],
                  "storageChanges": [
                    {
                      "account": "0x6ae08857a7ed8f5550e6b887d15c6e9754409298",
                      "key": "0x00100000000000000000000001f256d9fd1fbb4b514784584557751b0e2f8118",
                      "value": "0x9895b"
                    },
                    {
                      "account": "0x6ae08857a7ed8f5550e6b887d15c6e9754409298",
                      "key": "0x00100000000000000000000004206a77410c2a11a6a1adfb2f456eb9f4b2672b",
                      "value": "0xc"
                    }
                  ],
                  "receipt": {
                    "legacy": {
                      "gasUsed": "0x655db",
                      "logBloom": "0x63a018088011107060cab16088303230382034c20107f9f633326111644258831f0000",
                      "logs": [
                        {
                          "address": "0xc6004fbd8437201472f6d6dff362dbc4233f03f1",
                          "topics": [
                            "0xddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef",
                            "0x0000000000000000000000004af013afbadb22d8a88c92d68fc96b033b9ebb8a",
                            "0x000000000000000000000000cafe0854989c15301de5cf580a39015de48df4f8"
                          ],
                          "data": "0x00000000000000000000000000000000000000000000000000000000000003e8"
                        }
                      ],
                      "outcome": {
                        "statusCode": 1
                      }
                    }
                  }
                }            
              ]
        }"#;

        let deserialized = restore_frozen_state(std::io::Cursor::new(serialized_block)).unwrap();
        assert_eq!(deserialized.len(), 1);

        let known_block = deserialized.get(&8519876u64);
        assert_eq!(known_block.is_some(), true);

        if let Some(transactions) = known_block {
            assert_eq!(transactions.len(), 1);

            let tx = transactions.first();
            assert_eq!(tx.is_some(), true);

            if let Some(tx) = tx {
                assert_eq!(
                    tx.id,
                    H256::from_str(
                        "5c09d643b9f6a7cf9065e1ee2f47223ff74592428cb1181b4286145d4925504c"
                    )
                    .unwrap()
                );

                assert_eq!(
                    tx.balance_ops[0].account,
                    Address::from_str("1f256d9fd1fbb4b514784584557751b0e2f81185").unwrap()
                );
                assert_eq!(
                    tx.balance_ops[0].amount,
                    U256::from_str("2b6a067727c000").unwrap()
                );
                assert_eq!(tx.balance_ops[0].op, Op::Sub);

                assert_eq!(
                    tx.balance_ops[1].account,
                    Address::from_str("1f256d9fd1fbb4b514784584557751b0e2f81185").unwrap()
                );
                assert_eq!(
                    tx.balance_ops[1].amount,
                    U256::from_str("28983165f80a00").unwrap()
                );
                assert_eq!(tx.balance_ops[1].op, Op::Add);

                assert_eq!(
                    tx.balance_ops[2].account,
                    Address::from_str("0010f94b296a852aaac52ea6c5ac72e03afd032d").unwrap()
                );
                assert_eq!(
                    tx.balance_ops[2].amount,
                    U256::from_str("2d1d5112fb600").unwrap()
                );
                assert_eq!(tx.balance_ops[2].op, Op::Add);

                assert_eq!(
                    tx.storage_changes[0].account,
                    Address::from_str("6ae08857a7ed8f5550e6b887d15c6e9754409298").unwrap()
                );

                assert_eq!(
                    tx.storage_changes[0].key,
                    H256::from_str(
                        "00100000000000000000000001f256d9fd1fbb4b514784584557751b0e2f8118"
                    )
                    .unwrap()
                );

                assert_eq!(
                    tx.storage_changes[0].value,
                    U256::from_str("9895b").unwrap()
                );

                assert_eq!(
                    tx.storage_changes[1].account,
                    Address::from_str("6ae08857a7ed8f5550e6b887d15c6e9754409298").unwrap()
                );

                assert_eq!(
                    tx.storage_changes[1].key,
                    H256::from_str(
                        "00100000000000000000000004206a77410c2a11a6a1adfb2f456eb9f4b2672b"
                    )
                    .unwrap()
                );

                assert_eq!(tx.storage_changes[1].value, U256::from_str("c").unwrap());

                if let TypedReceipt::Legacy(ref receipt) = tx.receipt {
                    assert_eq!(receipt.gas_used, U256::from_str("655db").unwrap());
                    assert_eq!(receipt.outcome, TransactionOutcome::StatusCode(1));
                    assert_eq!(receipt.logs.len(), 1);
                } else {
                    assert!(false);
                }
            }
        }
    }
}
