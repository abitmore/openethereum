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

//! State test transaction deserialization.

use common_types::transaction::{
    Action, SignedTransaction, Transaction as CoreTransaction, TypedTransaction,
};

use ethkey::Secret;

use crate::{
    bytes::Bytes,
    hash::{Address, H256},
    maybe::MaybeEmpty,
    uint::Uint,
};

/// State test transaction deserialization.
#[derive(Debug, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Transaction {
    /// Transaction data.
    pub data: Bytes,
    /// Gas limit.
    pub gas_limit: Uint,
    /// Gas price.
    pub gas_price: Uint,
    /// Nonce.
    pub nonce: Uint,
    /// Secret key.
    #[serde(rename = "secretKey")]
    pub secret: Option<H256>,
    /// To.
    pub to: MaybeEmpty<Address>,
    /// Value.
    pub value: Uint,
}

impl From<Transaction> for SignedTransaction {
    fn from(t: Transaction) -> Self {
        let to: Option<Address> = t.to.into();
        let secret = t.secret.map(|s| Secret::from(s.0));
        let tx = TypedTransaction::Legacy(CoreTransaction {
            nonce: t.nonce.into(),
            gas_price: t.gas_price.into(),
            gas: t.gas_limit.into(),
            action: match to {
                Some(to) => Action::Call(to.into()),
                None => Action::Create,
            },
            value: t.value.into(),
            data: t.data.into(),
        });
        match secret {
            Some(s) => tx.sign(&Secret::from(s), None),
            None => tx.null_sign(1),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Transaction;
    use serde_json;

    #[test]
    fn transaction_deserialization() {
        let s = r#"{
			"data" : "",
			"accessLists": null,
			"gasLimit" : "0x2dc6c0",
			"gasPrice" : "0x01",
			"nonce" : "0x00",
			"secretKey" : "45a915e4d060149eb4365960e6a7a45f334393093061116b197e3240065ff2d8",
			"to" : "1000000000000000000000000000000000000000",
			"value" : "0x00"
		}"#;
        let _deserialized: Transaction = serde_json::from_str(s).unwrap();
        // TODO: validate all fields
    }
}
