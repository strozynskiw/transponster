use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Clone)]
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
    pub client: u16,
    #[serde(rename = "tx")]
    pub transaction_id: u32,
    pub amount: Option<Decimal>,
}

#[derive(Debug, PartialEq)]
pub struct AccountData {
    pub locked: bool,
    pub available: Decimal,
    pub held: Decimal,
    pub disputes: Vec<u32>,
}

#[derive(Debug, Serialize)]
pub struct ReportRow {
    pub client: u16,
    pub available: String,
    pub held: String,
    pub total: String,
    pub locked: bool,
}
