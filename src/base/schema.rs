// This file is part of TRINCI.
//
// Copyright (C) 2021 Affidaty Spa.
//
// TRINCI is free software: you can redistribute it and/or modify it under
// the terms of the GNU Affero General Public License as published by the
// Free Software Foundation, either version 3 of the License, or (at your
// option) any later version.
//
// TRINCI is distributed in the hope that it will be useful, but WITHOUT
// ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or
// FITNESS FOR A PARTICULAR PURPOSE. See the GNU Affero General Public License
// for more details.
//
// You should have received a copy of the GNU Affero General Public License
// along with TRINCI. If not, see <https://www.gnu.org/licenses/>.

use crate::{
    base::serialize::MessagePack,
    crypto::{Hash, Hashable, KeyPair, PublicKey},
    Error, ErrorKind, Result,
};
use serde_bytes::ByteBuf;
use std::collections::BTreeMap;

/// Transaction payload.
#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct TransactionDataV1 {
    /// Transaction schema version (TODO: is this necessary?).
    pub schema: String,
    /// Target account identifier.
    pub account: String,
    /// Max allowed blockchain asset units for fee.
    pub fuel_limit: u64,
    /// Nonce to differentiate different transactions with same payload.
    #[serde(with = "serde_bytes")]
    pub nonce: Vec<u8>,
    /// Network identifier.
    pub network: String,
    /// Expected smart contract application identifier.
    pub contract: Option<Hash>,
    /// Method name.
    pub method: String,
    /// Submitter public key.
    pub caller: PublicKey,
    /// Smart contract arguments.
    #[serde(with = "serde_bytes")]
    pub args: Vec<u8>,
}

/// Transaction payload for bulk node tx.
#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct TransactionDataBulkNodeV1 {
    pub schema: String,
    /// Target account identifier.
    pub account: String,
    /// Max allowed blockchain asset units for fee.
    pub fuel_limit: u64,
    /// Nonce to differentiate different transactions with same payload.
    #[serde(with = "serde_bytes")]
    pub nonce: Vec<u8>,
    /// Network identifier.
    pub network: String,
    /// Expected smart contract application identifier.
    pub contract: Option<Hash>,
    /// Method name.
    pub method: String,
    /// Submitter public key.
    pub caller: PublicKey,
    /// Smart contract arguments.
    #[serde(with = "serde_bytes")]
    pub args: Vec<u8>,
    /// It express the tx on which is dependant
    // TODO: change transaction, check box if valid solution
    pub depends_on: Hash,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
/// Set of transactions inside a bulk transaction
pub struct BulkTransactions {
    // is box right approach?
    pub root: Box<UnsignedTransaction>,
    pub nodes: Option<Vec<Transaction>>,
}

/// Transaction payload for bulk tx.
#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct TransactionDataBulkV1 {
    pub schema: String,
    /// array of transactions
    pub txs: BulkTransactions,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
#[serde(tag = "type")]
pub enum TransactionData {
    #[serde(rename = "v1")]
    V1(TransactionDataV1),
    #[serde(rename = "bnv1")]
    BulkNodeV1(TransactionDataBulkNodeV1),
    #[serde(rename = "brv1")]
    BulkRootV1(TransactionDataV1),
    #[serde(rename = "bv1")]
    BulkV1(TransactionDataBulkV1),
}

impl TransactionData {
    /// Transaction data sign
    pub fn sign(&self, keypair: &KeyPair) -> Result<Vec<u8>> {
        match &self {
            TransactionData::V1(tx_data) => tx_data.sign(keypair),
            TransactionData::BulkNodeV1(tx_data) => tx_data.sign(keypair),
            TransactionData::BulkV1(tx_data) => tx_data.sign(keypair),
            _ => Err(Error::new_ext(
                ErrorKind::NotImplemented,
                "signature method not implemented for this tx data type",
            )),
        }
    }
    /// Transaction data signature verification.
    pub fn verify(&self, public_key: &PublicKey, sig: &[u8]) -> Result<()> {
        match &self {
            TransactionData::V1(tx_data) => tx_data.verify(public_key, sig),
            TransactionData::BulkNodeV1(tx_data) => tx_data.verify(public_key, sig),
            TransactionData::BulkV1(tx_data) => tx_data.verify(public_key, sig),
            _ => Err(Error::new_ext(
                ErrorKind::NotImplemented,
                "verify method not implemented for this tx data type",
            )),
        }
    }
    /// Transaction data integrity check.
    pub fn check_integrity(&self) -> Result<()> {
        match &self {
            TransactionData::BulkV1(tx_data) => tx_data.check_integrity(),
            TransactionData::V1(tx_data) => tx_data.check_integrity(),
            _ => Err(Error::new_ext(
                ErrorKind::NotImplemented,
                "verify method not implemented for this tx data type",
            )),
        }
    }

    pub fn get_caller(&self) -> &PublicKey {
        match &self {
            TransactionData::V1(tx_data) => &tx_data.caller,
            TransactionData::BulkNodeV1(tx_data) => &tx_data.caller,
            TransactionData::BulkRootV1(tx_data) => &tx_data.caller,
            TransactionData::BulkV1(tx_data) => tx_data.txs.root.data.get_caller(),
        }
    }
    pub fn get_network(&self) -> &str {
        match &self {
            TransactionData::V1(tx_data) => &tx_data.network,
            TransactionData::BulkNodeV1(tx_data) => &tx_data.network,
            TransactionData::BulkRootV1(tx_data) => &tx_data.network,
            TransactionData::BulkV1(tx_data) => tx_data.txs.root.data.get_network(),
        }
    }
    pub fn get_account(&self) -> &str {
        match &self {
            TransactionData::V1(tx_data) => &tx_data.account,
            TransactionData::BulkNodeV1(tx_data) => &tx_data.account,
            TransactionData::BulkRootV1(tx_data) => &tx_data.account,
            TransactionData::BulkV1(tx_data) => tx_data.txs.root.data.get_account(),
        }
    }
    pub fn get_method(&self) -> &str {
        match &self {
            TransactionData::V1(tx_data) => &tx_data.method,
            TransactionData::BulkNodeV1(tx_data) => &tx_data.method,
            TransactionData::BulkRootV1(tx_data) => &tx_data.method,
            TransactionData::BulkV1(tx_data) => tx_data.txs.root.data.get_method(),
        }
    }
    pub fn get_args(&self) -> &[u8] {
        match &self {
            TransactionData::V1(tx_data) => &tx_data.args,
            TransactionData::BulkNodeV1(tx_data) => &tx_data.args,
            TransactionData::BulkRootV1(tx_data) => &tx_data.args,
            TransactionData::BulkV1(tx_data) => tx_data.txs.root.data.get_args(),
        }
    }
    pub fn get_contract(&self) -> &Option<Hash> {
        match &self {
            TransactionData::V1(tx_data) => &tx_data.contract,
            TransactionData::BulkNodeV1(tx_data) => &tx_data.contract,
            TransactionData::BulkRootV1(tx_data) => &tx_data.contract,
            TransactionData::BulkV1(tx_data) => tx_data.txs.root.data.get_contract(),
        }
    }
    pub fn get_dependency(&self) -> Result<Hash> {
        match &self {
            TransactionData::BulkNodeV1(tx_data) => Ok(tx_data.depends_on),
            _ => Err(Error::new_ext(
                ErrorKind::NotImplemented,
                "verify method not implemented for this tx data type",
            )),
        }
    }
    pub fn set_contract(&mut self, contract: Option<Hash>) {
        match self {
            TransactionData::V1(tx_data) => tx_data.contract = contract,
            TransactionData::BulkNodeV1(tx_data) => tx_data.contract = contract,
            TransactionData::BulkRootV1(tx_data) => tx_data.contract = contract,
            TransactionData::BulkV1(tx_data) => tx_data.txs.root.data.set_contract(contract),
        }
    }
    pub fn set_account(&mut self, account: String) {
        match self {
            TransactionData::V1(tx_data) => tx_data.account = account,
            TransactionData::BulkNodeV1(tx_data) => tx_data.account = account,
            TransactionData::BulkRootV1(tx_data) => tx_data.account = account,
            TransactionData::BulkV1(tx_data) => tx_data.txs.root.data.set_account(account),
        }
    }
    pub fn set_nonce(&mut self, nonce: Vec<u8>) {
        match self {
            TransactionData::V1(tx_data) => tx_data.nonce = nonce,
            TransactionData::BulkNodeV1(tx_data) => tx_data.nonce = nonce,
            TransactionData::BulkRootV1(tx_data) => tx_data.nonce = nonce,
            TransactionData::BulkV1(tx_data) => tx_data.txs.root.data.set_nonce(nonce),
        }
    }
}

impl TransactionDataV1 {
    /// Sign transaction data.
    /// Serialization is performed using message pack format with named field.
    pub fn sign(&self, keypair: &KeyPair) -> Result<Vec<u8>> {
        let data = self.serialize();
        keypair.sign(&data)
    }

    /// Transaction data signature verification.
    pub fn verify(&self, public_key: &PublicKey, sig: &[u8]) -> Result<()> {
        let data = self.serialize();
        match public_key.verify(&data, sig) {
            true => Ok(()),
            false => Err(ErrorKind::InvalidSignature.into()),
        }
    }

    /// Check if tx is intact and coherent
    pub fn check_integrity(&self) -> Result<()> {
        if !self.schema.is_empty()
            && !self.account.is_empty()
            && !self.nonce.is_empty()
            && !self.network.is_empty()
            && !self.method.is_empty()
        {
            return Ok(());
        } else {
            return Err(ErrorKind::BrokenIntegrity.into());
        }
    }
}

impl TransactionDataBulkNodeV1 {
    /// Sign transaction data.
    /// Serialization is performed using message pack format with named field.
    pub fn sign(&self, keypair: &KeyPair) -> Result<Vec<u8>> {
        let data = self.serialize();
        keypair.sign(&data)
    }

    /// Transaction data signature verification.
    pub fn verify(&self, public_key: &PublicKey, sig: &[u8]) -> Result<()> {
        let data = self.serialize();
        match public_key.verify(&data, sig) {
            true => Ok(()),
            false => Err(ErrorKind::InvalidSignature.into()),
        }
    }
}

impl TransactionDataBulkV1 {
    /// Sign transaction data.
    /// Serialization is performed using message pack format with named field.
    pub fn sign(&self, keypair: &KeyPair) -> Result<Vec<u8>> {
        let data = self.serialize();
        keypair.sign(&data)
    }

    /// Transaction data signature verification.
    // it sould take the public key of the first tx
    // check sign
    pub fn verify(&self, public_key: &PublicKey, sig: &[u8]) -> Result<()> {
        let data = self.serialize();
        match public_key.verify(&data, sig) {
            true => match &self.txs.nodes {
                Some(nodes) => {
                    for node in nodes {
                        match node {
                            Transaction::UnitTransaction(node) => match &node.data {
                                TransactionData::BulkNodeV1(data) => {
                                    let result = data.verify(public_key, sig);
                                    if result.is_err() {
                                        return Err(ErrorKind::InvalidSignature.into());
                                    }
                                }
                                _ => return Err(ErrorKind::WrongTxType.into()),
                            },
                            Transaction::BullkTransaction(_) => {
                                return Err(ErrorKind::WrongTxType.into())
                            }
                        }
                    }
                    return Ok(());
                }
                None => Ok(()),
            },
            false => Err(ErrorKind::InvalidSignature.into()),
        }
    }

    /// It checks that all the txs are intact and coherent
    pub fn check_integrity(&self) -> Result<()> {
        // calculate root hash
        let root_hash = self
            .txs
            .root
            .data
            .hash(crate::crypto::HashAlgorithm::Sha256);
        let network = self.txs.root.data.get_network();
        match &self.txs.nodes {
            Some(nodes) => {
                // check depens on
                // check nws all equals && != none
                for node in nodes {
                    match node {
                        Transaction::UnitTransaction(tx) => {
                            // check depends_on filed
                            let dependency = tx.data.get_dependency();
                            match dependency {
                                Ok(dep_hash) => {
                                    if dep_hash != root_hash {
                                        return Err(Error::new_ext(
                                            ErrorKind::BrokenIntegrity,
                                            "The node has incoherent dependency",
                                        ));
                                    }
                                }
                                Err(error) => return Err(error),
                            }

                            // check network field
                            if tx.data.get_network() != network {
                                return Err(Error::new_ext(
                                    ErrorKind::BrokenIntegrity,
                                    "The node has incoherent network",
                                ));
                            }
                        }
                        Transaction::BullkTransaction(_) => {
                            return Err(ErrorKind::WrongTxType.into())
                        }
                    }
                }

                Ok(())
            }
            None => Ok(()),
        }
    }
}

/// Signed transaction.
#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct SignedTransaction {
    /// Transaction payload.
    pub data: TransactionData,
    /// Data field signature verifiable using the `caller` within the `data`.
    #[serde(with = "serde_bytes")]
    pub signature: Vec<u8>,
}

/// Unsigned Transaction
#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct UnsignedTransaction {
    /// Transaction payload.
    pub data: TransactionData,
}

/// Bulk Transaction
// it might not be needed, just use signed transaction, where data == transaction data::bulkdata
#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct BulkTransaction {
    /// Transaction payload.
    pub data: TransactionData,
    /// Data field signature verifiable using the `caller` within the `data`.
    #[serde(with = "serde_bytes")]
    pub signature: Vec<u8>,
}

/// Enum for transaction types
#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub enum Transaction {
    /// Unit signed transaction
    UnitTransaction(SignedTransaction),
    /// Bulk transaction
    BullkTransaction(BulkTransaction),
}

impl Transaction {
    pub fn sign(&self, keypair: &KeyPair) -> Result<Vec<u8>> {
        match self {
            Transaction::UnitTransaction(tx) => tx.data.sign(keypair),
            Transaction::BullkTransaction(tx) => tx.data.sign(keypair),
        }
    }
    pub fn verify(&self, public_key: &PublicKey, sig: &[u8]) -> Result<()> {
        match self {
            Transaction::UnitTransaction(tx) => tx.data.verify(public_key, sig),
            Transaction::BullkTransaction(tx) => tx.data.verify(public_key, sig),
        }
    }
    pub fn check_integrity(&self) -> Result<()> {
        match self {
            Transaction::UnitTransaction(tx) => tx.data.check_integrity(), //TODO
            Transaction::BullkTransaction(tx) => tx.data.check_integrity(),
        }
    }

    pub fn get_caller(&self) -> &PublicKey {
        match self {
            Transaction::UnitTransaction(tx) => tx.data.get_caller(),
            Transaction::BullkTransaction(tx) => tx.data.get_caller(),
        }
    }
    pub fn get_network(&self) -> &str {
        match &self {
            Transaction::UnitTransaction(tx) => tx.data.get_network(),
            Transaction::BullkTransaction(tx) => tx.data.get_network(),
        }
    }
    pub fn get_account(&self) -> &str {
        match &self {
            Transaction::UnitTransaction(tx) => tx.data.get_account(),
            Transaction::BullkTransaction(tx) => tx.data.get_account(),
        }
    }
    pub fn get_method(&self) -> &str {
        match &self {
            Transaction::UnitTransaction(tx) => tx.data.get_method(),
            Transaction::BullkTransaction(tx) => tx.data.get_method(),
        }
    }
    pub fn get_args(&self) -> &[u8] {
        match &self {
            Transaction::UnitTransaction(tx) => tx.data.get_args(),
            Transaction::BullkTransaction(tx) => tx.data.get_args(),
        }
    }
    pub fn get_contract(&self) -> &Option<Hash> {
        match &self {
            Transaction::UnitTransaction(tx) => tx.data.get_contract(),
            Transaction::BullkTransaction(tx) => tx.data.get_contract(),
        }
    }
    pub fn get_dependency(&self) -> Result<Hash> {
        match &self {
            Transaction::UnitTransaction(tx) => tx.data.get_dependency(),
            Transaction::BullkTransaction(tx) => tx.data.get_dependency(),
        }
    }
    pub fn get_signature(&self) -> &Vec<u8> {
        match &self {
            Transaction::UnitTransaction(tx) => &tx.signature,
            Transaction::BullkTransaction(tx) => &tx.signature,
        }
    }
}

/// Events risen by the smart contract execution
#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct SmartContractEvent {
    /// Identifier of the transaction that produced this event
    pub event_tx: Hash,

    /// The account that produced this event
    pub emitter_account: String,

    /// Arbitrary name given to this event
    pub event_name: String,

    /// Data emitted with this event
    #[serde(with = "serde_bytes")]
    pub event_data: Vec<u8>,
}

/// Transaction execution receipt.
#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct Receipt {
    /// Transaction block location.
    pub height: u64,
    /// Transaction index within the block.
    pub index: u32,
    /// Actual burned fuel used to perform the submitted actions.
    pub burned_fuel: u64,
    /// Execution outcome.
    pub success: bool,
    // Follows contract specific result data.
    #[serde(with = "serde_bytes")]
    pub returns: Vec<u8>,
    /// Optional Vector of smart contract events
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub events: Option<Vec<SmartContractEvent>>,
}

/// Block structure.
#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct Block {
    /// Index in the blockhain, which is also the number of ancestors blocks.
    pub height: u64,
    /// Number of transactions in this block.
    pub size: u32,
    /// Previous block hash.
    pub prev_hash: Hash,
    /// Root of block transactions trie.
    pub txs_hash: Hash,
    /// Root of block receipts trie.
    pub rxs_hash: Hash,
    /// Root of accounts state after applying the block transactions.
    pub state_hash: Hash,
}

impl Block {
    /// Instance a new block structure.
    pub fn new(
        height: u64,
        size: u32,
        prev_hash: Hash,
        txs_hash: Hash,
        rxs_hash: Hash,
        state_hash: Hash,
    ) -> Self {
        Block {
            height,
            size,
            prev_hash,
            txs_hash,
            rxs_hash,
            state_hash,
        }
    }
}

/// Account structure.
#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct Account {
    /// Account identifier.
    pub id: String,
    /// Assets map.
    pub assets: BTreeMap<String, ByteBuf>,
    /// Associated smart contract application hash (wasm binary hash).
    pub contract: Option<Hash>,
    /// Merkle tree root of the data associated with the account.
    pub data_hash: Option<Hash>,
}

impl Account {
    /// Creates a new account by associating to it the owner's public key and a
    /// contract unique identifier (wasm binary sha-256).
    pub fn new(id: &str, contract: Option<Hash>) -> Account {
        Account {
            id: id.to_owned(),
            assets: BTreeMap::new(),
            contract,
            data_hash: None,
        }
    }

    /// Get account balance for the given asset.
    pub fn load_asset(&self, asset: &str) -> Vec<u8> {
        self.assets
            .get(asset)
            .cloned()
            .unwrap_or_default()
            .into_vec()
    }

    /// Set account balance for the given asset.
    pub fn store_asset(&mut self, asset: &str, value: &[u8]) {
        let buf = ByteBuf::from(value);
        self.assets.insert(asset.to_string(), buf);
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::{
        base::serialize::MessagePack,
        crypto::{
            ecdsa::tests::{ecdsa_secp384_test_keypair, ecdsa_secp384_test_public_key},
            Hashable,
        },
        ErrorKind,
    };

    const ACCOUNT_ID: &str = "QmNLei78zWmzUdbeRB3CiUfAizWUrbeeZh5K1rhAQKCh51";

    const TRANSACTION_DATA_HEX_UNIT: &str = "9aa27631ae6d792d636f6f6c2d736368656d61d92e516d59486e45514c64663568374b59626a4650754853526b325350676458724a5746683557363936485066713769cd03e8c408ab82b741e023a412a6736b796e6574c42212202c26b46b68ffc68ff99b453c1d30413413422d706483bfa0f98a5e886266e7aea97465726d696e61746593a56563647361a9736563703338347231c461045936d631b849bb5760bcf62e0d1261b6b6e227dc0a3892cbeec91be069aaa25996f276b271c2c53cba4be96d67edcadd66b793456290609102d5401f413cd1b5f4130b9cfaa68d30d0d25c3704cb72734cd32064365ff7042f5a3eee09b06cc1c40a4f706171756544617461";
    const TRANSACTION_DATA_HASH_HEX_UNIT: &str =
        "1220b27267c4cf81983ec9785e594bd9b6ede6d207cebd0c6b4032c6823a96784cc0";

    const TRANSACTION_DATA_HEX_BULK: &str = "93a3627631ae6d792d636f6f6c2d736368656d6192919aa462727631ae6d792d636f6f6c2d736368656d61d92e516d59486e45514c64663568374b59626a4650754853526b325350676458724a5746683557363936485066713769cd03e8c408ab82b741e023a412a6736b796e6574c42212202c26b46b68ffc68ff99b453c1d30413413422d706483bfa0f98a5e886266e7aea97465726d696e61746593a56563647361a9736563703338347231c461045936d631b849bb5760bcf62e0d1261b6b6e227dc0a3892cbeec91be069aaa25996f276b271c2c53cba4be96d67edcadd66b793456290609102d5401f413cd1b5f4130b9cfaa68d30d0d25c3704cb72734cd32064365ff7042f5a3eee09b06cc1c40a4f706171756544617461c0";
    const TRANSACTION_DATA_HASH_HEX_BULK: &str =
        "12205ba4b7698ccbd0f662c5f64de7aba4f9a86a869c1ef8acd6120b7684a126e48c";

    const TRANSACTION_HEX_UNIT: &str = "8100929aa27631ae6d792d636f6f6c2d736368656d61d92e516d59486e45514c64663568374b59626a4650754853526b325350676458724a5746683557363936485066713769cd03e8c408ab82b741e023a412a6736b796e6574c42212202c26b46b68ffc68ff99b453c1d30413413422d706483bfa0f98a5e886266e7aea97465726d696e61746593a56563647361a9736563703338347231c461045936d631b849bb5760bcf62e0d1261b6b6e227dc0a3892cbeec91be069aaa25996f276b271c2c53cba4be96d67edcadd66b793456290609102d5401f413cd1b5f4130b9cfaa68d30d0d25c3704cb72734cd32064365ff7042f5a3eee09b06cc1c40a4f706171756544617461c460cf2665db3c17f94579404a7a87204960446f7d65a7962db22953721576bf125a72215bfdee464bf025d2359615550fa6660cc53fb729b02ef251c607dfc93dc441a783bb058c41e694fe99904969f69d0735a794dc85010e4156a6edcb55177e";
    const TRANSACTION_SIGN_UNIT: &str = "cf2665db3c17f94579404a7a87204960446f7d65a7962db22953721576bf125a72215bfdee464bf025d2359615550fa6660cc53fb729b02ef251c607dfc93dc441a783bb058c41e694fe99904969f69d0735a794dc85010e4156a6edcb55177e";

    const TRANSACTION_HEX_BULK: &str = "81019293a3627631ae6d792d636f6f6c2d736368656d6192919aa462727631ae6d792d636f6f6c2d736368656d61d92e516d59486e45514c64663568374b59626a4650754853526b325350676458724a5746683557363936485066713769cd03e8c408ab82b741e023a412a6736b796e6574c42212202c26b46b68ffc68ff99b453c1d30413413422d706483bfa0f98a5e886266e7aea97465726d696e61746593a56563647361a9736563703338347231c461045936d631b849bb5760bcf62e0d1261b6b6e227dc0a3892cbeec91be069aaa25996f276b271c2c53cba4be96d67edcadd66b793456290609102d5401f413cd1b5f4130b9cfaa68d30d0d25c3704cb72734cd32064365ff7042f5a3eee09b06cc1c40a4f706171756544617461c0c460bc09b9742f3927593aa88e9be7fefea5f44529571f64e2535e3a1917ca63812baca39171fc7d85cc2dc437607ebc5c23554a5ff4d4dd3900abbfbb4f841007d38c99dfc1b0e54d4b5d0b266d17534ce2f50d97d09296a653669ed7840c8a5d63";
    const TRANSACTION_SIGN_BULK: &str = "bc09b9742f3927593aa88e9be7fefea5f44529571f64e2535e3a1917ca63812baca39171fc7d85cc2dc437607ebc5c23554a5ff4d4dd3900abbfbb4f841007d38c99dfc1b0e54d4b5d0b266d17534ce2f50d97d09296a653669ed7840c8a5d63";

    const RECEIPT_HEX: &str = "960309cd03e7c3c40a4f70617175654461746190";
    const RECEIPT_HASH_HEX: &str =
        "12202da9df047846a2c30866388c0650a1b126c421f4f3b55bea254edc1b4281cac3";

    const BLOCK_HEX: &str = "960103c4221220648263253df78db6c2f1185e832c546f2f7a9becbdc21d3be41c80dc96b86011c4221220f937696c204cc4196d48f3fe7fc95c80be266d210b95397cc04cfc6b062799b8c4221220dec404bd222542402ffa6b32ebaa9998823b7bb0a628152601d1da11ec70b867c422122005db394ef154791eed2cb97e7befb2864a5702ecfd44fab7ef1c5ca215475c7d";
    const BLOCK_HASH_HEX: &str =
        "12207967613f2ce65f93437c8da954ba4a32a795dc1235ff179cd27f92f330521ccb";

    const ACCOUNT_CONTRACT_HEX: &str = "94d92e516d4e4c656937387a576d7a556462655242334369556641697a5755726265655a68354b31726841514b4368353181a3534b59c40103c422122087b6239079719fc7e4349ec54baac9e04c20c48cf0c6a9d2b29b0ccf7c31c727c0";
    const ACCOUNT_NCONTRAC_HEX: &str = "94d92e516d4e4c656937387a576d7a556462655242334369556641697a5755726265655a68354b31726841514b4368353181a3534b59c40103c0c0";

    const CONTRACT_EVENT_HEX: &str = "94c42212202c26b46b68ffc68ff99b453c1d30413413422d706483bfa0f98a5e886266e7aeae6f726967696e5f6163636f756e74ab636f6f6c5f6d6574686f64c403010203";

    const TRANSACTION_SCHEMA: &str = "my-cool-schema";
    const FUEL_LIMIT: u64 = 1000;

    fn create_test_data_unit() -> TransactionData {
        // Opaque information returned by the smart contract.
        let args = hex::decode("4f706171756544617461").unwrap();
        let public_key = PublicKey::Ecdsa(ecdsa_secp384_test_public_key());
        let account = public_key.to_account_id();
        let contract =
            Hash::from_hex("12202c26b46b68ffc68ff99b453c1d30413413422d706483bfa0f98a5e886266e7ae")
                .unwrap();

        TransactionData::V1(TransactionDataV1 {
            schema: TRANSACTION_SCHEMA.to_owned(),
            account,
            fuel_limit: FUEL_LIMIT,
            nonce: [0xab, 0x82, 0xb7, 0x41, 0xe0, 0x23, 0xa4, 0x12].to_vec(),
            network: "skynet".to_string(),
            contract: Some(contract),
            method: "terminate".to_string(),
            caller: public_key,
            args,
        })
    }

    fn create_test_data_bulk() -> TransactionData {
        // Opaque information returned by the smart contract.
        let args = hex::decode("4f706171756544617461").unwrap();
        let public_key = PublicKey::Ecdsa(ecdsa_secp384_test_public_key());
        let account = public_key.to_account_id();
        let contract =
            Hash::from_hex("12202c26b46b68ffc68ff99b453c1d30413413422d706483bfa0f98a5e886266e7ae")
                .unwrap();

        let root_data = TransactionData::BulkRootV1(TransactionDataV1 {
            schema: TRANSACTION_SCHEMA.to_owned(),
            account,
            fuel_limit: FUEL_LIMIT,
            nonce: [0xab, 0x82, 0xb7, 0x41, 0xe0, 0x23, 0xa4, 0x12].to_vec(),
            network: "skynet".to_string(),
            contract: Some(contract),
            method: "terminate".to_string(),
            caller: public_key,
            args,
        });

        let root = UnsignedTransaction { data: root_data };

        TransactionData::BulkV1(TransactionDataBulkV1 {
            schema: TRANSACTION_SCHEMA.to_owned(),
            txs: BulkTransactions {
                root: Box::new(root),
                nodes: None,
            },
        })
    }

    pub fn create_test_unit_tx() -> Transaction {
        let signature = hex::decode(TRANSACTION_SIGN_UNIT).unwrap();
        Transaction::UnitTransaction(SignedTransaction {
            data: create_test_data_unit(),
            signature,
        })
    }

    pub fn create_test_bulk_tx() -> Transaction {
        let signature = hex::decode(TRANSACTION_SIGN_BULK).unwrap();
        Transaction::BullkTransaction(BulkTransaction {
            data: create_test_data_bulk(),
            signature,
        })
    }

    pub fn create_test_contract_event() -> SmartContractEvent {
        SmartContractEvent {
            event_tx: Hash::from_hex(
                "12202c26b46b68ffc68ff99b453c1d30413413422d706483bfa0f98a5e886266e7ae",
            )
            .unwrap(),
            emitter_account: "origin_account".to_string(),
            event_name: "cool_method".to_string(),
            event_data: vec![1, 2, 3],
        }
    }

    pub fn create_test_receipt() -> Receipt {
        // Opaque information returned by the smart contract.
        let returns = hex::decode("4f706171756544617461").unwrap();
        Receipt {
            height: 3,
            index: 9,
            burned_fuel: 999,
            success: true,
            returns,
            events: Some(Vec::new()),
        }
    }

    pub fn create_test_account() -> Account {
        let hash =
            Hash::from_hex("122087b6239079719fc7e4349ec54baac9e04c20c48cf0c6a9d2b29b0ccf7c31c727")
                .unwrap();
        let mut account = Account::new(ACCOUNT_ID, Some(hash));
        account
            .assets
            .insert("SKY".to_string(), ByteBuf::from([3u8].to_vec()));
        account
    }

    pub fn create_test_block() -> Block {
        let prev_hash =
            Hash::from_hex("1220648263253df78db6c2f1185e832c546f2f7a9becbdc21d3be41c80dc96b86011")
                .unwrap();
        let txs_hash =
            Hash::from_hex("1220f937696c204cc4196d48f3fe7fc95c80be266d210b95397cc04cfc6b062799b8")
                .unwrap();
        let res_hash =
            Hash::from_hex("1220dec404bd222542402ffa6b32ebaa9998823b7bb0a628152601d1da11ec70b867")
                .unwrap();
        let state_hash =
            Hash::from_hex("122005db394ef154791eed2cb97e7befb2864a5702ecfd44fab7ef1c5ca215475c7d")
                .unwrap();
        Block {
            height: 1,
            size: 3,
            prev_hash,
            txs_hash,
            rxs_hash: res_hash,
            state_hash,
        }
    }

    #[test]
    fn transaction_data_serialize_unit() {
        let data = create_test_data_unit();

        let buf = data.serialize();

        assert_eq!(TRANSACTION_DATA_HEX_UNIT, hex::encode(buf));
    }

    #[test]
    fn transaction_data_serialize_bulk() {
        let data = create_test_data_bulk();

        let buf = data.serialize();

        assert_eq!(TRANSACTION_DATA_HEX_BULK, hex::encode(buf));
    }

    #[test]
    fn transaction_data_deserialize_unit() {
        let expected = create_test_data_unit();

        let buf = hex::decode(TRANSACTION_DATA_HEX_UNIT).unwrap();

        let data = TransactionData::deserialize(&buf).unwrap();

        assert_eq!(expected, data);
    }

    #[test]
    fn transaction_data_deserialize_bulk() {
        let expected = create_test_data_bulk();

        let buf = hex::decode(TRANSACTION_DATA_HEX_BULK).unwrap();

        let data = TransactionData::deserialize(&buf).unwrap();

        assert_eq!(expected, data);
    }

    #[test]
    fn transaction_data_deserialize_fail_unit() {
        let mut buf = hex::decode(TRANSACTION_DATA_HEX_UNIT).unwrap();
        buf.pop(); // remove a byte to make it fail

        let error = TransactionData::deserialize(&buf).unwrap_err();

        assert_eq!(error.kind, ErrorKind::MalformedData);
    }

    #[test]
    fn transaction_data_deserialize_fail_bulk() {
        let mut buf = hex::decode(TRANSACTION_DATA_HEX_BULK).unwrap();
        buf.pop(); // remove a byte to make it fail

        let error = TransactionData::deserialize(&buf).unwrap_err();

        assert_eq!(error.kind, ErrorKind::MalformedData);
    }

    #[test]
    fn transaction_data_hash() {
        let tx = create_test_unit_tx();
        let hash = match tx {
            Transaction::UnitTransaction(tx) => tx.data.primary_hash(),
            Transaction::BullkTransaction(tx) => tx.data.primary_hash(),
        };
        assert_eq!(TRANSACTION_DATA_HASH_HEX_UNIT, hex::encode(hash));

        let tx = create_test_bulk_tx();
        let hash = match tx {
            Transaction::UnitTransaction(tx) => tx.data.primary_hash(),
            Transaction::BullkTransaction(tx) => tx.data.primary_hash(),
        };
        assert_eq!(TRANSACTION_DATA_HASH_HEX_BULK, hex::encode(hash));
    }

    #[test]
    fn transaction_data_verify() {
        let tx = create_test_unit_tx();
        let result = tx.verify(tx.get_caller(), tx.get_signature());
        assert!(result.is_ok());

        let tx = create_test_bulk_tx();

        // to know sign
        //let ser = match tx {
        //    Transaction::UnitTransaction(tx) => tx.data.serialize(),
        //    Transaction::BullkTransaction(tx) => match tx.data {
        //        TransactionData::BulkV1(data) => data.serialize(),
        //        _ => tx.data.serialize(),
        //    },
        //};

        //print!(
        //    "{:?}",
        //    hex::encode(ecdsa_secp384_test_keypair().sign(&ser).unwrap())
        //);

        let result = tx.verify(tx.get_caller(), tx.get_signature());
        assert!(result.is_ok());
    }

    #[test]
    fn unit_transaction_data_sign_verify() {
        let data = create_test_data_unit();
        let keypair = KeyPair::Ecdsa(ecdsa_secp384_test_keypair());

        let signature = data.sign(&keypair).unwrap();
        let result = data.verify(&keypair.public_key(), &signature);

        println!("SIGN: {}", hex::encode(&signature));
        assert!(result.is_ok());
    }

    #[test]
    fn bulk_transaction_data_sign_verify() {
        let data = create_test_data_bulk();
        let keypair = KeyPair::Ecdsa(ecdsa_secp384_test_keypair());

        let signature = data.sign(&keypair).unwrap();
        let result = data.verify(&keypair.public_key(), &signature);

        println!("SIGN: {}", hex::encode(&signature));
        assert!(result.is_ok());
    }

    #[test]
    fn transaction_serialize() {
        let tx = create_test_unit_tx();

        let buf = tx.serialize();

        assert_eq!(TRANSACTION_HEX_UNIT, hex::encode(buf));

        let tx = create_test_bulk_tx();

        let buf = tx.serialize();

        assert_eq!(TRANSACTION_HEX_BULK, hex::encode(buf));
    }

    #[test]
    fn transaction_deserialize() {
        let expected = create_test_unit_tx();
        let buf = hex::decode(TRANSACTION_HEX_UNIT).unwrap();

        let tx = Transaction::deserialize(&buf).unwrap();

        assert_eq!(expected, tx);

        let expected = create_test_bulk_tx();
        let buf = hex::decode(TRANSACTION_HEX_BULK).unwrap();

        let tx = Transaction::deserialize(&buf).unwrap();

        assert_eq!(expected, tx);
    }

    #[test]
    fn transaction_deserialize_fail() {
        let mut buf = hex::decode(TRANSACTION_HEX_UNIT).unwrap();
        buf.pop();

        let error = Transaction::deserialize(&buf).unwrap_err();

        assert_eq!(error.kind, ErrorKind::MalformedData);

        let mut buf = hex::decode(TRANSACTION_HEX_BULK).unwrap();
        buf.pop();

        let error = Transaction::deserialize(&buf).unwrap_err();

        assert_eq!(error.kind, ErrorKind::MalformedData);
    }

    #[test]
    fn receipt_serialize() {
        let receipt = create_test_receipt();

        let buf = receipt.serialize();

        assert_eq!(hex::encode(buf), RECEIPT_HEX);
    }

    #[test]
    fn receipt_deserialize() {
        let expected = create_test_receipt();
        let buf = hex::decode(RECEIPT_HEX).unwrap();

        let receipt = Receipt::deserialize(&buf).unwrap();

        assert_eq!(receipt, expected);
    }

    #[test]
    fn receipt_deserialize_fail() {
        let mut buf = hex::decode(RECEIPT_HEX).unwrap();
        buf.pop(); // remove a byte to make it fail

        let err = Receipt::deserialize(&buf).unwrap_err();

        assert_eq!(err.kind, ErrorKind::MalformedData);
    }

    #[test]
    fn receipt_hash() {
        let receipt = create_test_receipt();

        let hash = receipt.primary_hash();

        assert_eq!(RECEIPT_HASH_HEX, hex::encode(hash));
    }

    #[test]
    fn contract_event_serialize() {
        let event = create_test_contract_event();

        let buf = event.serialize();

        assert_eq!(hex::encode(buf), CONTRACT_EVENT_HEX);
    }

    #[test]
    fn contract_event_deserialize() {
        let expected = create_test_contract_event();
        let buf = hex::decode(CONTRACT_EVENT_HEX).unwrap();

        let event = SmartContractEvent::deserialize(&buf).unwrap();

        assert_eq!(event, expected);
    }

    #[test]
    fn block_serialize() {
        let block = create_test_block();

        let buf = block.serialize();

        assert_eq!(hex::encode(buf), BLOCK_HEX);
    }

    #[test]
    fn block_deserialize() {
        let expected = create_test_block();
        let buf = hex::decode(BLOCK_HEX).unwrap();

        let block = Block::deserialize(&buf).unwrap();

        assert_eq!(block, expected);
    }

    #[test]
    fn block_deserialize_fail() {
        let mut buf = hex::decode(BLOCK_HEX).unwrap();
        buf.pop(); // remove a byte to make it fail

        let error = Block::deserialize(&buf).unwrap_err();

        assert_eq!(error.kind, ErrorKind::MalformedData);
    }

    #[test]
    fn block_hash() {
        let block = create_test_block();

        let hash = block.primary_hash();

        assert_eq!(BLOCK_HASH_HEX, hex::encode(hash));
    }

    #[test]
    fn account_serialize() {
        let account = create_test_account();

        let buf = account.serialize();

        assert_eq!(hex::encode(buf), ACCOUNT_CONTRACT_HEX);
    }

    #[test]
    fn account_deserialize() {
        let expected = create_test_account();
        let buf = hex::decode(ACCOUNT_CONTRACT_HEX).unwrap();

        let account = Account::deserialize(&buf).unwrap();

        assert_eq!(account, expected);
    }

    #[test]
    fn account_serialize_null_contract() {
        let mut account = create_test_account();
        account.contract = None;

        let buf = account.serialize();

        assert_eq!(hex::encode(buf), ACCOUNT_NCONTRAC_HEX);
    }

    #[test]
    fn account_deserialize_null_contract() {
        let mut expected = create_test_account();
        expected.contract = None;
        let buf = hex::decode(ACCOUNT_NCONTRAC_HEX).unwrap();

        let account = Account::deserialize(&buf).unwrap();

        assert_eq!(account, expected);
    }

    #[test]
    fn account_deserialize_fail() {
        let mut buf = hex::decode(ACCOUNT_CONTRACT_HEX).unwrap();
        buf.pop();

        let error = Account::deserialize(&buf).unwrap_err();

        assert_eq!(error.kind, ErrorKind::MalformedData);
    }

    #[test]
    fn account_store_asset() {
        let mut account = create_test_account();

        account.store_asset("BTC", &[3]);

        assert_eq!(account.load_asset("BTC"), [3]);
    }

    #[test]
    fn account_load_asset() {
        let mut account = create_test_account();
        account.store_asset("BTC", &[3]);

        let value = account.load_asset("BTC");

        assert_eq!(value, [3]);
    }
}
