#[derive(Debug)]
pub enum OperationType {
    Deposite,
    Withdrawal,
    Dispute,
    Resolve,
    Chargeback,
    Unknown,
}

#[derive(Debug)]
pub struct Transaction {
    pub transaction_id: u32,
    pub operation: OperationType,
    pub client: u16,
    pub amount: Option<i64>,
}

#[derive(Debug, PartialEq)]
pub struct AccountData {
    pub locked: bool,
    pub available: i64,
    pub held: i64,
    pub disputes: Vec<u32>,
}
