use std::collections::{HashMap, HashSet};

use indexmap::IndexMap;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use strum::Display;

pub type ClientId = u16;
pub type TransactionId = u32;

pub type AccountsMap = IndexMap<ClientId, AccountData>;

#[derive(Debug, Deserialize, Clone, Display, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum OperationType {
    Deposit,
    Withdrawal,
    Dispute,
    Resolve,
    Chargeback,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Transaction {
    #[serde(rename = "type")]
    pub operation: OperationType,
    #[serde(rename = "client")]
    pub client_id: ClientId,
    #[serde(rename = "tx")]
    pub id: TransactionId,

    // None if not provided at all
    #[serde(default)]
    pub amount: Option<Decimal>,
}

#[derive(Debug)]
pub struct AccountData {
    pub locked: bool,
    pub available: Decimal,
    pub held: Decimal,

    pub transactions: HashMap<TransactionId, Transaction>,
    pub under_dispute: HashSet<TransactionId>,
}

impl PartialEq for AccountData {
    fn eq(&self, other: &Self) -> bool {
        (self.locked == other.locked)
            && (self.available == other.available)
            && (self.held == other.held)
            && (self.under_dispute == other.under_dispute)
    }
}

impl Default for AccountData {
    fn default() -> Self {
        Self {
            available: Decimal::ZERO,
            held: Decimal::ZERO,
            locked: false,
            under_dispute: HashSet::new(),
            transactions: HashMap::new(),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct ReportRow {
    #[serde(rename = "client")]
    pub client_id: ClientId,
    pub available: Decimal,
    pub held: Decimal,
    pub total: Decimal,
    pub locked: bool,
}
