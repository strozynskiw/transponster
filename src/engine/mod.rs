use anyhow::Result;
use csv::{ReaderBuilder, Trim};
use rust_decimal_macros::dec;
use std::{collections::HashMap, fs::File, path::Path};

pub mod models;
use models::{AccountData, OperationType, Transaction};

use thiserror::Error;

pub type AccountsMap = HashMap<u16, AccountData>;
pub type Transactions = Vec<Transaction>;

pub struct Engine {
    accounts: AccountsMap,

    // That should be just a database access so i don't care about the vector
    // inefficiency in that role
    transactions: Transactions,
}

#[derive(Error, Debug)]
pub enum ProcessorError {
    #[error("Parsing error")]
    Parsing(#[from] csv::Error),
    #[error("IO read error")]
    Reading(#[from] std::io::Error),
    //#[error("Engine operation error: `{0}`")]
    //Operation(String),
}

impl Engine {
    pub fn new() -> Engine {
        Self {
            accounts: AccountsMap::new(),
            transactions: Transactions::new(),
        }
    }

    pub fn process_input(&mut self, path: &Path) -> Result<(), ProcessorError> {
        let file = File::open(path)?;
        let mut rdr = ReaderBuilder::new().trim(Trim::All).from_reader(file);
        for line in rdr.deserialize() {
            let transaction: Transaction = line?;
            self.process_one(transaction)?;
        }

        Ok(())
    }

    pub fn print_report(&self) {
        // Some buf writer here would be great, but let's keep it simple.
        println!("client,available,held,total,locked");
        self.accounts.iter().for_each(|(c, d)| {
            println!(
                "{},{:.04},{:.04},{:.04},{}",
                c,
                d.available,
                d.held,
                d.available + d.held,
                d.locked
            )
        });
    }

    fn get_account(&mut self, client_id: u16) -> &mut AccountData {
        self.accounts.entry(client_id).or_insert(AccountData {
            available: dec!(0),
            held: dec!(0),
            locked: false,
            disputes: Vec::new(),
        })
    }

    fn process_one(&mut self, transaction: Transaction) -> Result<(), ProcessorError> {
        match transaction.operation {
            OperationType::Deposit => self.operation_deposit(&transaction)?,
            OperationType::Withdrawal => self.operation_withdraw(&transaction)?,
            OperationType::Dispute => self.operation_dispute(&transaction)?,
            OperationType::Resolve => self.operation_resolve(&transaction)?,
            OperationType::Chargeback => self.operation_chargeback(&transaction)?,
        }
        self.transactions.push(transaction);
        Ok(())
    }

    fn operation_deposit(&mut self, details: &Transaction) -> Result<(), ProcessorError> {
        let account = self.get_account(details.client);

        if account.locked {
            return Ok(());
        }

        if let Some(v) = details.amount {
            account.available += v;
        } // Ignored as there is nothing to do

        Ok(())
    }

    fn operation_withdraw(&mut self, details: &Transaction) -> Result<(), ProcessorError> {
        let account = self.get_account(details.client);

        if account.locked {
            return Ok(());
        }

        if let Some(v) = details.amount {
            if account.available >= v {
                account.available -= v;
            } // I don't want to fail the whole operation processing here, so it is ignored.
        } // Ignored as there is nothing to do

        Ok(())
    }

    fn operation_dispute(&mut self, details: &Transaction) -> Result<(), ProcessorError> {
        let referenced_transaction = self
            .transactions
            .iter()
            .find(|x| x.transaction_id == details.transaction_id)
            .cloned();

        let account = self.get_account(details.client);

        if account.locked {
            return Ok(());
        }

        if let Some(t) = referenced_transaction {
            match t.amount {
                Some(amount)
                    if details.client == t.client
                        && !account.disputes.contains(&details.transaction_id) =>
                {
                    match t.operation {
                        OperationType::Deposit => {
                            account.available -= amount;
                            account.held += amount;
                        }
                        OperationType::Withdrawal => {
                            account.held += amount;
                        }
                        _ => {} // Should not happen
                    }

                    account.disputes.push(details.transaction_id);
                }
                Some(_) => {} // Client doesn't match or dispute already exists
                None => {}    // No amount
            }
        } // No referenced transaction

        Ok(())
    }

    fn operation_resolve(&mut self, details: &Transaction) -> Result<(), ProcessorError> {
        let referenced_transaction = self
            .transactions
            .iter()
            .find(|x| x.transaction_id == details.transaction_id)
            .cloned();

        let account = self.get_account(details.client);

        if account.locked {
            return Ok(());
        }

        if let Some(t) = referenced_transaction {
            match t.amount {
                Some(amount)
                    if details.client == t.client
                        && account.disputes.contains(&details.transaction_id) =>
                {
                    match t.operation {
                        OperationType::Deposit => {
                            account.available += amount;
                            account.held -= amount;
                        }
                        OperationType::Withdrawal => {
                            account.held -= amount;
                        }
                        _ => {} // Invalid input ignored
                    }

                    if let Some(index) = account
                        .disputes
                        .iter()
                        .position(|x| *x == details.transaction_id)
                    {
                        account.disputes.remove(index);
                    } // Else ignored - third party issue
                }
                Some(_) => {} // client doesn't match or dispute doesn't exist
                None => {}    // No amount
            }
        } // No referenced transaction - third party error
        Ok(())
    }

    fn operation_chargeback(&mut self, details: &Transaction) -> Result<(), ProcessorError> {
        let referenced_transaction = self
            .transactions
            .iter()
            .find(|x| x.transaction_id == details.transaction_id)
            .cloned();

        let account = self.get_account(details.client);

        if account.locked {
            return Ok(());
        }

        if let Some(t) = referenced_transaction {
            match t.amount {
                Some(amount)
                    if details.client == t.client
                        && account.disputes.contains(&details.transaction_id) =>
                {
                    match t.operation {
                        OperationType::Deposit => {
                            account.held -= amount;
                        }
                        OperationType::Withdrawal => {
                            account.available += amount;
                            account.held -= amount;
                        }
                        _ => {} // Should not happen
                    }

                    let index = account
                        .disputes
                        .iter()
                        .position(|x| *x == details.transaction_id)
                        .unwrap();
                    account.disputes.remove(index);
                    account.locked = true;
                }
                Some(_) => {} // Client doesn't match or dispute doesn't exist
                None => {}    // No amount
            }
        } // No referenced transaction
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use rust_decimal_macros::dec;

    use crate::engine::models::AccountData;
    use crate::engine::models::OperationType;

    use super::Transaction;
    use super::Transactions;

    #[test]
    fn two_deposits() {
        let transactions: Transactions = vec![
            Transaction {
                transaction_id: 1,
                operation: OperationType::Deposit,
                client: 10,
                amount: Some(dec!(1)),
            },
            Transaction {
                transaction_id: 2,
                operation: OperationType::Deposit,
                client: 10,
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
                disputes: vec![]
            },
            engine.accounts.get(&10).unwrap()
        );
    }

    #[test]
    fn two_withdrawals() {
        let transactions: Transactions = vec![
            Transaction {
                transaction_id: 1,
                operation: OperationType::Withdrawal,
                client: 10,
                amount: Some(dec!(1)),
            },
            Transaction {
                transaction_id: 2,
                operation: OperationType::Withdrawal,
                client: 10,
                amount: Some(dec!(1)),
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
                locked: false,
                disputes: vec![]
            },
            engine.accounts.get(&10).unwrap()
        );
    }

    #[test]
    fn deposit_withdraw_balance_positive() {
        let transactions: Transactions = vec![
            Transaction {
                transaction_id: 1,
                operation: OperationType::Deposit,
                client: 10,
                amount: Some(dec!(1)),
            },
            Transaction {
                transaction_id: 2,
                operation: OperationType::Withdrawal,
                client: 10,
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
                disputes: vec![]
            },
            engine.accounts.get(&10).unwrap()
        );
    }

    #[test]
    fn deposit_withdraw_balance_negative() {
        let transactions: Transactions = vec![
            Transaction {
                transaction_id: 1,
                operation: OperationType::Deposit,
                client: 10,
                amount: Some(dec!(1)),
            },
            Transaction {
                transaction_id: 2,
                operation: OperationType::Withdrawal,
                client: 10,
                amount: Some(dec!(1.5)),
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
                disputes: vec![]
            },
            engine.accounts.get(&10).unwrap()
        );
    }

    #[test]
    fn deposit_and_dispute() {
        let transactions: Transactions = vec![
            Transaction {
                transaction_id: 1,
                operation: OperationType::Deposit,
                client: 10,
                amount: Some(dec!(1)),
            },
            Transaction {
                transaction_id: 1,
                operation: OperationType::Dispute,
                client: 10,
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
                held: dec!(1),
                locked: false,
                disputes: vec![1]
            },
            engine.accounts.get(&10).unwrap()
        );
    }

    #[test]
    fn deposit_withdrawal_and_dispute_deposit() {
        let transactions: Transactions = vec![
            Transaction {
                transaction_id: 1,
                operation: OperationType::Deposit,
                client: 10,
                amount: Some(dec!(3)),
            },
            Transaction {
                transaction_id: 2,
                operation: OperationType::Withdrawal,
                client: 10,
                amount: Some(dec!(2)),
            },
            Transaction {
                transaction_id: 1,
                operation: OperationType::Dispute,
                client: 10,
                amount: None,
            },
        ];

        let mut engine = super::Engine::new();
        transactions
            .into_iter()
            .for_each(|t| engine.process_one(t).unwrap());

        assert_eq!(
            &AccountData {
                available: dec!(-2),
                held: dec!(3),
                locked: false,
                disputes: vec![1]
            },
            engine.accounts.get(&10).unwrap()
        );
    }

    #[test]
    fn deposit_dispute_and_resolve_deposit() {
        let transactions: Transactions = vec![
            Transaction {
                transaction_id: 1,
                operation: OperationType::Deposit,
                client: 10,
                amount: Some(dec!(1)),
            },
            Transaction {
                transaction_id: 1,
                operation: OperationType::Dispute,
                client: 10,
                amount: None,
            },
            Transaction {
                transaction_id: 1,
                operation: OperationType::Resolve,
                client: 10,
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
                disputes: vec![]
            },
            engine.accounts.get(&10).unwrap()
        );
    }

    #[test]
    fn deposit_dispute_and_chargeback_deposit() {
        let transactions: Transactions = vec![
            Transaction {
                transaction_id: 1,
                operation: OperationType::Deposit,
                client: 10,
                amount: Some(dec!(1)),
            },
            Transaction {
                transaction_id: 1,
                operation: OperationType::Dispute,
                client: 10,
                amount: None,
            },
            Transaction {
                transaction_id: 1,
                operation: OperationType::Chargeback,
                client: 10,
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
                disputes: vec![]
            },
            engine.accounts.get(&10).unwrap()
        );
    }

    #[test]
    fn deposit_withdrawal_and_dispute_withdrawal() {
        let transactions: Transactions = vec![
            Transaction {
                transaction_id: 1,
                operation: OperationType::Deposit,
                client: 10,
                amount: Some(dec!(2)),
            },
            Transaction {
                transaction_id: 2,
                operation: OperationType::Withdrawal,
                client: 10,
                amount: Some(dec!(1)),
            },
            Transaction {
                transaction_id: 2,
                operation: OperationType::Dispute,
                client: 10,
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
                disputes: vec![2]
            },
            engine.accounts.get(&10).unwrap()
        );
    }

    #[test]
    fn deposit_withdrawal_dispute_and_chargeback_withdrawal() {
        let transactions: Transactions = vec![
            Transaction {
                transaction_id: 1,
                operation: OperationType::Deposit,
                client: 10,
                amount: Some(dec!(2)),
            },
            Transaction {
                transaction_id: 2,
                operation: OperationType::Withdrawal,
                client: 10,
                amount: Some(dec!(1)),
            },
            Transaction {
                transaction_id: 2,
                operation: OperationType::Dispute,
                client: 10,
                amount: None,
            },
            Transaction {
                transaction_id: 2,
                operation: OperationType::Chargeback,
                client: 10,
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
                locked: true,
                disputes: vec![]
            },
            engine.accounts.get(&10).unwrap()
        );
    }

    #[test]
    fn deposit_withdrawal_dispute_and_chargeback_deposit() {
        let transactions: Transactions = vec![
            Transaction {
                transaction_id: 1,
                operation: OperationType::Deposit,
                client: 10,
                amount: Some(dec!(2)),
            },
            Transaction {
                transaction_id: 2,
                operation: OperationType::Withdrawal,
                client: 10,
                amount: Some(dec!(1)),
            },
            Transaction {
                transaction_id: 1,
                operation: OperationType::Dispute,
                client: 10,
                amount: None,
            },
            Transaction {
                transaction_id: 1,
                operation: OperationType::Chargeback,
                client: 10,
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
                disputes: vec![]
            },
            engine.accounts.get(&10).unwrap()
        );
    }

    #[test]
    fn deposit_withdrawal_dispute_and_resolve_deposit() {
        let transactions: Transactions = vec![
            Transaction {
                transaction_id: 1,
                operation: OperationType::Deposit,
                client: 10,
                amount: Some(dec!(2)),
            },
            Transaction {
                transaction_id: 2,
                operation: OperationType::Withdrawal,
                client: 10,
                amount: Some(dec!(1)),
            },
            Transaction {
                transaction_id: 1,
                operation: OperationType::Dispute,
                client: 10,
                amount: None,
            },
            Transaction {
                transaction_id: 1,
                operation: OperationType::Resolve,
                client: 10,
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
                disputes: vec![]
            },
            engine.accounts.get(&10).unwrap()
        );
    }

    #[test]
    fn deposit_withdrawal_dispute_and_resolve_withdrawal() {
        let transactions: Transactions = vec![
            Transaction {
                transaction_id: 1,
                operation: OperationType::Deposit,
                client: 10,
                amount: Some(dec!(2)),
            },
            Transaction {
                transaction_id: 2,
                operation: OperationType::Withdrawal,
                client: 10,
                amount: Some(dec!(1)),
            },
            Transaction {
                transaction_id: 2,
                operation: OperationType::Dispute,
                client: 10,
                amount: None,
            },
            Transaction {
                transaction_id: 2,
                operation: OperationType::Resolve,
                client: 10,
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
                disputes: vec![]
            },
            engine.accounts.get(&10).unwrap()
        );
    }

    #[test]
    fn no_deposit_on_locked_account() {
        let transactions: Transactions = vec![
            Transaction {
                transaction_id: 1,
                operation: OperationType::Deposit,
                client: 10,
                amount: Some(dec!(2)),
            },
            Transaction {
                transaction_id: 1,
                operation: OperationType::Dispute,
                client: 10,
                amount: None,
            },
            Transaction {
                transaction_id: 1,
                operation: OperationType::Chargeback,
                client: 10,
                amount: None,
            },
            Transaction {
                transaction_id: 2,
                operation: OperationType::Deposit,
                client: 10,
                amount: Some(dec!(2)),
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
                disputes: vec![]
            },
            engine.accounts.get(&10).unwrap()
        );
    }

    #[test]
    fn no_withdrawal_on_locked_account() {
        let transactions: Transactions = vec![
            Transaction {
                transaction_id: 1,
                operation: OperationType::Deposit,
                client: 10,
                amount: Some(dec!(2)),
            },
            Transaction {
                transaction_id: 1,
                operation: OperationType::Dispute,
                client: 10,
                amount: None,
            },
            Transaction {
                transaction_id: 1,
                operation: OperationType::Chargeback,
                client: 10,
                amount: None,
            },
            Transaction {
                transaction_id: 2,
                operation: OperationType::Withdrawal,
                client: 10,
                amount: Some(dec!(2)),
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
                disputes: vec![]
            },
            engine.accounts.get(&10).unwrap()
        );
    }
}
