use anyhow::Result;
use csv::{Reader, ReaderBuilder, Trim, Writer};
use rust_decimal::Decimal;

use std::path::PathBuf;

pub mod error;
use error::{EngineError, ProcessingError};

pub mod models;
use models::{AccountData, AccountsMap, OperationType, ReportRow, Transaction};

pub struct Engine {
    accounts: AccountsMap,
}

impl Engine {
    pub fn new() -> Self {
        Self {
            accounts: AccountsMap::new(),
        }
    }

    // This public method takes file to load.
    pub fn process_input(&mut self, path: &PathBuf) -> Result<(), EngineError> {
        let rdr = ReaderBuilder::new()
            .flexible(true)
            .trim(Trim::All)
            .from_path(path)?;
        self.process_from_reader(rdr)
    }

    // This is extracted mostly for parsing test purposes but could also be used with other sources that just a file
    pub fn process_from_reader<T: std::io::Read>(
        &mut self,
        mut reader: Reader<T>,
    ) -> Result<(), EngineError> {
        for line in reader.deserialize() {
            let transaction: Transaction = line?;

            // That's how return processing error wrapped with EngineError
            // This however stops the execution.
            // self.process_one(transaction)?;

            if let Err(e) = self.process_one(transaction) {
                eprintln!("Processing error: {e}");
            }
        }

        Ok(())
    }

    pub fn serialize_report_to_writer<T: std::io::Write>(
        &self,
        mut writer: Writer<T>,
    ) -> Result<(), EngineError> {
        self.accounts
            .iter()
            .map(|(client_id, data)| ReportRow {
                client_id: *client_id,
                available: data.available,
                held: data.held,
                total: data.available + data.held,
                locked: data.locked,
            })
            .try_for_each(|row| writer.serialize(row))?;

        writer.flush()?;

        Ok(())
    }

    pub fn serialize_report_stdout(&mut self) -> Result<(), EngineError> {
        let writer = csv::Writer::from_writer(std::io::stdout());
        self.serialize_report_to_writer(writer)
    }
    fn process_one(&mut self, transaction: Transaction) -> Result<(), ProcessingError> {
        let account = self.accounts.entry(transaction.client_id).or_default();

        if account.locked {
            return Err(ProcessingError::AccountLocked(transaction.client_id));
        };

        match transaction.operation {
            OperationType::Deposit => operation_deposit(account, transaction)?,
            OperationType::Withdrawal => operation_withdraw(account, transaction)?,
            OperationType::Dispute => operation_dispute(account, transaction)?,
            OperationType::Resolve => operation_resolve(account, transaction)?,
            OperationType::Chargeback => operation_chargeback(account, transaction)?,
        }

        Ok(())
    }
}

fn operation_deposit(
    account: &mut AccountData,
    transaction: Transaction,
) -> Result<(), ProcessingError> {
    // Deduplication
    if account.transactions.contains_key(&transaction.id) {
        return Err(ProcessingError::DuplicatedTransaction(
            transaction.id,
            transaction.client_id,
        ));
    }

    let amount = transaction
        .amount
        .ok_or(ProcessingError::MissingAmount(transaction.id))?;

    if amount < Decimal::ZERO {
        return Err(ProcessingError::NegativeAmount);
    }

    account.available = account
        .available
        .checked_add(amount)
        .ok_or(ProcessingError::Overflow(transaction.id))?;

    account.transactions.insert(transaction.id, transaction);

    Ok(())
}

fn operation_withdraw(
    account: &mut AccountData,
    transaction: Transaction,
) -> Result<(), ProcessingError> {
    // Deduplication
    if account.transactions.contains_key(&transaction.id) {
        return Err(ProcessingError::DuplicatedTransaction(
            transaction.id,
            transaction.client_id,
        ));
    }

    let amount = transaction
        .amount
        .ok_or(ProcessingError::MissingAmount(transaction.id))?;

    if amount < Decimal::ZERO {
        return Err(ProcessingError::NegativeAmount);
    }

    if account.available < amount {
        return Err(ProcessingError::InsufficientFounds(
            transaction.id,
            transaction.client_id,
        ));
    }

    account.available = account
        .available
        .checked_sub(amount)
        .ok_or(ProcessingError::Underflow(transaction.id))?;

    account.transactions.insert(transaction.id, transaction);

    Ok(())
}

fn operation_dispute(
    account: &mut AccountData,
    transaction: Transaction,
) -> Result<(), ProcessingError> {
    let referenced_transaction = account.transactions.get(&transaction.id);

    let disputed_transaction =
        referenced_transaction.ok_or(ProcessingError::MissingTransaction(transaction.id))?;

    // Check duplicated dispute for a transaction
    if account.under_dispute.contains(&disputed_transaction.id) {
        return Err(ProcessingError::DuplicatedDispute(
            transaction.id,
            disputed_transaction.id,
            transaction.client_id,
        ));
    }

    let disputed_amount = disputed_transaction
        .amount
        .ok_or(ProcessingError::MissingAmount(transaction.id))?;

    match disputed_transaction.operation {
        OperationType::Deposit => {
            // We need to do both checked operations to keep the transaction valid
            let new_available = account
                .available
                .checked_sub(disputed_amount)
                .ok_or(ProcessingError::Underflow(transaction.id))?;

            let new_held = account
                .held
                .checked_add(disputed_amount)
                .ok_or(ProcessingError::Overflow(transaction.id))?;

            account.available = new_available;
            account.held = new_held;
        }
        OperationType::Withdrawal => {
            // The other way around. I guess it means withdrawn money was
            // not received, so we put it back for now
            account.held = account
                .held
                .checked_add(disputed_amount)
                .ok_or(ProcessingError::Overflow(transaction.id))?;
        }
        _ => {
            return Err(ProcessingError::InvalidOperationUnderDispute(
                transaction.operation,
                transaction.id,
            ))
        }
    }

    account.under_dispute.insert(disputed_transaction.id);

    Ok(())
}

fn operation_resolve(
    account: &mut AccountData,
    transaction: Transaction,
) -> Result<(), ProcessingError> {
    let referenced_transaction = account.transactions.get(&transaction.id);

    let disputed_transaction =
        referenced_transaction.ok_or(ProcessingError::MissingTransaction(transaction.id))?;

    // Check if transaction under dispute
    if !account.under_dispute.contains(&disputed_transaction.id) {
        return Err(ProcessingError::IncorrectResolve(
            transaction.operation,
            transaction.id,
        ));
    }

    let disputed_amount = disputed_transaction
        .amount
        .ok_or(ProcessingError::MissingAmount(transaction.id))?;

    match disputed_transaction.operation {
        OperationType::Deposit | OperationType::Withdrawal => {
            let new_available = account
                .available
                .checked_add(disputed_amount)
                .ok_or(ProcessingError::Overflow(transaction.id))?;

            let new_held = account
                .held
                .checked_sub(disputed_amount)
                .ok_or(ProcessingError::Underflow(transaction.id))?;

            account.available = new_available;
            account.held = new_held;
        }
        _ => {
            return Err(ProcessingError::InvalidOperationUnderDispute(
                transaction.operation,
                transaction.id,
            ))
        }
    }

    account.under_dispute.remove(&disputed_transaction.id);

    Ok(())
}

fn operation_chargeback(
    account: &mut AccountData,
    transaction: Transaction,
) -> Result<(), ProcessingError> {
    let referenced_transaction = account.transactions.get(&transaction.id);

    let disputed_transaction =
        referenced_transaction.ok_or(ProcessingError::MissingTransaction(transaction.id))?;

    // Check if transaction under dispute
    if !account.under_dispute.contains(&disputed_transaction.id) {
        return Err(ProcessingError::IncorrectChargeback(
            transaction.operation,
            transaction.id,
        ));
    }

    let disputed_amount = disputed_transaction
        .amount
        .ok_or(ProcessingError::MissingAmount(transaction.id))?;

    match disputed_transaction.operation {
        OperationType::Deposit | OperationType::Withdrawal => {
            account.held = account
                .held
                .checked_sub(disputed_amount)
                .ok_or(ProcessingError::Underflow(transaction.id))?;
        }
        _ => {
            return Err(ProcessingError::InvalidOperationUnderDispute(
                transaction.operation,
                transaction.id,
            ))
        }
    }

    account.under_dispute.remove(&disputed_transaction.id);

    account.locked = true;

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use rust_decimal_macros::dec;

    use crate::engine::error::ProcessingError;
    use crate::engine::models::AccountData;
    use crate::engine::models::OperationType;

    use super::Transaction;

    #[test]
    fn error_duplicated_transaction() {
        let mut engine = super::Engine::new();
        engine
            .process_one(Transaction {
                id: 1,
                operation: OperationType::Deposit,
                client_id: 10,
                amount: Some(dec!(1)),
            })
            .unwrap();

        let result = engine.process_one(Transaction {
            id: 1,
            operation: OperationType::Withdrawal,
            client_id: 10,
            amount: Some(dec!(2)),
        });

        assert_eq!(result, Err(ProcessingError::DuplicatedTransaction(1, 10)));
    }

    #[test]
    fn error_insufficient_founds() {
        let mut engine = super::Engine::new();
        engine
            .process_one(Transaction {
                id: 1,
                operation: OperationType::Deposit,
                client_id: 10,
                amount: Some(dec!(1)),
            })
            .unwrap();

        let result = engine.process_one(Transaction {
            id: 2,
            operation: OperationType::Withdrawal,
            client_id: 10,
            amount: Some(dec!(2)),
        });

        assert_eq!(result, Err(ProcessingError::InsufficientFounds(2, 10)));
    }

    #[test]
    fn two_deposits() {
        let transactions: Vec<Transaction> = vec![
            Transaction {
                id: 1,
                operation: OperationType::Deposit,
                client_id: 10,
                amount: Some(dec!(1)),
            },
            Transaction {
                id: 2,
                operation: OperationType::Deposit,
                client_id: 10,
                amount: Some(dec!(1)),
            },
        ];

        let mut engine = super::Engine::new();
        transactions
            .into_iter()
            .for_each(|t| engine.process_one(t).unwrap());

        assert_eq!(
            &AccountData {
                available: dec!(2),
                held: dec!(0),
                locked: false,
                ..Default::default()
            },
            engine.accounts.get(&10).unwrap()
        );
    }

    #[test]
    fn two_withdrawals() {
        let transactions: Vec<Transaction> = vec![
            Transaction {
                id: 1,
                operation: OperationType::Withdrawal,
                client_id: 10,
                amount: Some(dec!(1)),
            },
            Transaction {
                id: 2,
                operation: OperationType::Withdrawal,
                client_id: 10,
                amount: Some(dec!(1)),
            },
        ];

        let mut engine = super::Engine::new();
        transactions
            .into_iter()
            .for_each(|t| _ = engine.process_one(t));

        assert_eq!(
            &AccountData {
                available: dec!(0),
                held: dec!(0),
                locked: false,
                ..Default::default()
            },
            engine.accounts.get(&10).unwrap()
        );
    }

    #[test]
    fn deposit_withdraw_balance_positive() {
        let transactions: Vec<Transaction> = vec![
            Transaction {
                id: 1,
                operation: OperationType::Deposit,
                client_id: 10,
                amount: Some(dec!(1)),
            },
            Transaction {
                id: 2,
                operation: OperationType::Withdrawal,
                client_id: 10,
                amount: Some(dec!(0.5)),
            },
        ];

        let mut engine = super::Engine::new();
        transactions
            .into_iter()
            .for_each(|t| engine.process_one(t).unwrap());

        assert_eq!(
            &AccountData {
                available: dec!(0.5),
                held: dec!(0),
                locked: false,
                ..Default::default()
            },
            engine.accounts.get(&10).unwrap()
        );
    }

    #[test]
    fn deposit_withdraw_balance_negative() {
        let transactions: Vec<Transaction> = vec![
            Transaction {
                id: 1,
                operation: OperationType::Deposit,
                client_id: 10,
                amount: Some(dec!(1)),
            },
            Transaction {
                id: 2,
                operation: OperationType::Withdrawal,
                client_id: 10,
                amount: Some(dec!(1.5)),
            },
        ];

        let mut engine = super::Engine::new();
        transactions
            .into_iter()
            .for_each(|t| _ = engine.process_one(t));

        assert_eq!(
            &AccountData {
                available: dec!(1),
                held: dec!(0),
                locked: false,
                ..Default::default()
            },
            engine.accounts.get(&10).unwrap()
        );
    }

    #[test]
    fn deposit_and_dispute() {
        let transactions: Vec<Transaction> = vec![
            Transaction {
                id: 1,
                operation: OperationType::Deposit,
                client_id: 10,
                amount: Some(dec!(1)),
            },
            Transaction {
                id: 1,
                operation: OperationType::Dispute,
                client_id: 10,
                amount: None,
            },
        ];

        let mut engine = super::Engine::new();
        transactions
            .into_iter()
            .for_each(|t| _ = engine.process_one(t));

        assert_eq!(
            &AccountData {
                available: dec!(0),
                held: dec!(1),
                locked: false,
                under_dispute: HashSet::from_iter(vec![1]),
                ..Default::default()
            },
            engine.accounts.get(&10).unwrap()
        );
    }

    #[test]
    fn deposit_withdrawal_and_dispute_deposit() {
        let transactions: Vec<Transaction> = vec![
            Transaction {
                id: 1,
                operation: OperationType::Deposit,
                client_id: 10,
                amount: Some(dec!(3)),
            },
            Transaction {
                id: 2,
                operation: OperationType::Withdrawal,
                client_id: 10,
                amount: Some(dec!(2)),
            },
            Transaction {
                id: 1,
                operation: OperationType::Dispute,
                client_id: 10,
                amount: None,
            },
        ];

        let mut engine = super::Engine::new();
        transactions
            .into_iter()
            .for_each(|t| _ = engine.process_one(t));

        assert_eq!(
            &AccountData {
                available: dec!(-2),
                held: dec!(3),
                locked: false,
                under_dispute: HashSet::from_iter(vec![1]),
                ..Default::default()
            },
            engine.accounts.get(&10).unwrap()
        );
    }

    #[test]
    fn deposit_dispute_and_resolve_deposit() {
        let transactions: Vec<Transaction> = vec![
            Transaction {
                id: 1,
                operation: OperationType::Deposit,
                client_id: 10,
                amount: Some(dec!(1)),
            },
            Transaction {
                id: 1,
                operation: OperationType::Dispute,
                client_id: 10,
                amount: None,
            },
            Transaction {
                id: 1,
                operation: OperationType::Resolve,
                client_id: 10,
                amount: None,
            },
        ];

        let mut engine = super::Engine::new();
        transactions
            .into_iter()
            .for_each(|t| engine.process_one(t).unwrap());

        assert_eq!(
            &AccountData {
                available: dec!(1),
                held: dec!(0),
                locked: false,
                ..Default::default()
            },
            engine.accounts.get(&10).unwrap()
        );
    }

    #[test]
    fn deposit_dispute_and_chargeback_deposit() {
        let transactions: Vec<Transaction> = vec![
            Transaction {
                id: 1,
                operation: OperationType::Deposit,
                client_id: 10,
                amount: Some(dec!(1)),
            },
            Transaction {
                id: 1,
                operation: OperationType::Dispute,
                client_id: 10,
                amount: None,
            },
            Transaction {
                id: 1,
                operation: OperationType::Chargeback,
                client_id: 10,
                amount: None,
            },
        ];

        let mut engine = super::Engine::new();
        transactions
            .into_iter()
            .for_each(|t| engine.process_one(t).unwrap());

        assert_eq!(
            &AccountData {
                available: dec!(0),
                held: dec!(0),
                locked: true,
                ..Default::default()
            },
            engine.accounts.get(&10).unwrap()
        );
    }

    #[test]
    fn deposit_withdrawal_and_dispute_withdrawal() {
        let transactions: Vec<Transaction> = vec![
            Transaction {
                id: 1,
                operation: OperationType::Deposit,
                client_id: 10,
                amount: Some(dec!(2)),
            },
            Transaction {
                id: 2,
                operation: OperationType::Withdrawal,
                client_id: 10,
                amount: Some(dec!(1)),
            },
            Transaction {
                id: 2,
                operation: OperationType::Dispute,
                client_id: 10,
                amount: None,
            },
        ];

        let mut engine = super::Engine::new();
        transactions
            .into_iter()
            .for_each(|t| engine.process_one(t).unwrap());

        assert_eq!(
            &AccountData {
                available: dec!(1),
                held: dec!(1),
                locked: false,
                under_dispute: HashSet::from_iter(vec![2]),
                ..Default::default()
            },
            engine.accounts.get(&10).unwrap()
        );
    }

    #[test]
    fn deposit_withdrawal_dispute_and_chargeback_withdrawal() {
        let transactions: Vec<Transaction> = vec![
            Transaction {
                id: 1,
                operation: OperationType::Deposit,
                client_id: 10,
                amount: Some(dec!(2)),
            },
            Transaction {
                id: 2,
                operation: OperationType::Withdrawal,
                client_id: 10,
                amount: Some(dec!(1)),
            },
            Transaction {
                id: 2,
                operation: OperationType::Dispute,
                client_id: 10,
                amount: None,
            },
            Transaction {
                id: 2,
                operation: OperationType::Chargeback,
                client_id: 10,
                amount: None,
            },
        ];

        let mut engine = super::Engine::new();
        transactions
            .into_iter()
            .for_each(|t| engine.process_one(t).unwrap());

        assert_eq!(
            &AccountData {
                available: dec!(1),
                held: dec!(0),
                locked: true,
                ..Default::default()
            },
            engine.accounts.get(&10).unwrap()
        );
    }

    #[test]
    fn deposit_withdrawal_dispute_and_chargeback_deposit() {
        let transactions: Vec<Transaction> = vec![
            Transaction {
                id: 1,
                operation: OperationType::Deposit,
                client_id: 10,
                amount: Some(dec!(2)),
            },
            Transaction {
                id: 2,
                operation: OperationType::Withdrawal,
                client_id: 10,
                amount: Some(dec!(1)),
            },
            Transaction {
                id: 1,
                operation: OperationType::Dispute,
                client_id: 10,
                amount: None,
            },
            Transaction {
                id: 1,
                operation: OperationType::Chargeback,
                client_id: 10,
                amount: None,
            },
        ];

        let mut engine = super::Engine::new();
        transactions
            .into_iter()
            .for_each(|t| engine.process_one(t).unwrap());

        assert_eq!(
            &AccountData {
                available: dec!(-1),
                held: dec!(0),
                locked: true,
                ..Default::default()
            },
            engine.accounts.get(&10).unwrap()
        );
    }

    #[test]
    fn deposit_withdrawal_dispute_and_resolve_deposit() {
        let transactions: Vec<Transaction> = vec![
            Transaction {
                id: 1,
                operation: OperationType::Deposit,
                client_id: 10,
                amount: Some(dec!(2)),
            },
            Transaction {
                id: 2,
                operation: OperationType::Withdrawal,
                client_id: 10,
                amount: Some(dec!(1)),
            },
            Transaction {
                id: 1,
                operation: OperationType::Dispute,
                client_id: 10,
                amount: None,
            },
            Transaction {
                id: 1,
                operation: OperationType::Resolve,
                client_id: 10,
                amount: None,
            },
        ];

        let mut engine = super::Engine::new();
        transactions
            .into_iter()
            .for_each(|t| engine.process_one(t).unwrap());

        assert_eq!(
            &AccountData {
                available: dec!(1),
                held: dec!(0),
                locked: false,
                ..Default::default()
            },
            engine.accounts.get(&10).unwrap()
        );
    }

    #[test]
    fn deposit_withdrawal_dispute_and_resolve_withdrawal() {
        let transactions: Vec<Transaction> = vec![
            Transaction {
                id: 1,
                operation: OperationType::Deposit,
                client_id: 10,
                amount: Some(dec!(2)),
            },
            Transaction {
                id: 2,
                operation: OperationType::Withdrawal,
                client_id: 10,
                amount: Some(dec!(1)),
            },
            Transaction {
                id: 2,
                operation: OperationType::Dispute,
                client_id: 10,
                amount: None,
            },
            Transaction {
                id: 2,
                operation: OperationType::Resolve,
                client_id: 10,
                amount: None,
            },
        ];

        let mut engine = super::Engine::new();
        transactions
            .into_iter()
            .for_each(|t| engine.process_one(t).unwrap());

        assert_eq!(
            &AccountData {
                available: dec!(2),
                held: dec!(0),
                locked: false,
                ..Default::default()
            },
            engine.accounts.get(&10).unwrap()
        );
    }

    #[test]
    fn no_deposit_on_locked_account() {
        let transactions: Vec<Transaction> = vec![
            Transaction {
                id: 1,
                operation: OperationType::Deposit,
                client_id: 10,
                amount: Some(dec!(2)),
            },
            Transaction {
                id: 1,
                operation: OperationType::Dispute,
                client_id: 10,
                amount: None,
            },
            Transaction {
                id: 1,
                operation: OperationType::Chargeback,
                client_id: 10,
                amount: None,
            },
            Transaction {
                id: 2,
                operation: OperationType::Deposit,
                client_id: 10,
                amount: Some(dec!(2)),
            },
        ];

        let mut engine = super::Engine::new();
        transactions
            .into_iter()
            .for_each(|t| _ = engine.process_one(t));

        assert_eq!(
            &AccountData {
                available: dec!(0),
                held: dec!(0),
                locked: true,
                ..Default::default()
            },
            engine.accounts.get(&10).unwrap()
        );
    }

    #[test]
    fn no_withdrawal_on_locked_account() {
        let transactions: Vec<Transaction> = vec![
            Transaction {
                id: 1,
                operation: OperationType::Deposit,
                client_id: 10,
                amount: Some(dec!(2)),
            },
            Transaction {
                id: 1,
                operation: OperationType::Dispute,
                client_id: 10,
                amount: None,
            },
            Transaction {
                id: 1,
                operation: OperationType::Chargeback,
                client_id: 10,
                amount: None,
            },
            Transaction {
                id: 2,
                operation: OperationType::Withdrawal,
                client_id: 10,
                amount: Some(dec!(2)),
            },
        ];

        let mut engine = super::Engine::new();
        transactions
            .into_iter()
            .for_each(|t| _ = engine.process_one(t));

        assert_eq!(
            &AccountData {
                available: dec!(0),
                held: dec!(0),
                locked: true,
                ..Default::default()
            },
            engine.accounts.get(&10).unwrap()
        );
    }
}
