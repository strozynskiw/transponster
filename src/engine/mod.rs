use anyhow::Result;
use std::collections::HashMap;

pub mod models;
use models::{AccountData, OperationType, Transaction};

pub type AccountsMap = HashMap<u16, AccountData>;
pub type Transactions = Vec<Transaction>;

pub fn process(input: &Transactions) -> Result<AccountsMap> {
    let mut accounts_map: AccountsMap = HashMap::new();

    input
        .iter()
        .for_each(|t| process_one(t, input, &mut accounts_map));

    Ok(accounts_map)
}

pub fn process_one(
    transaction: &Transaction,
    transactions: &Transactions,
    accounts: &mut AccountsMap,
) {
    let mut account = accounts.entry(transaction.client).or_insert(AccountData {
        available: 0,
        held: 0,
        locked: false,
        disputes: Vec::new(),
    });

    if !account.locked {
        match transaction.operation {
            OperationType::Deposite => operation_deposit(transaction, &mut account),
            OperationType::Withdrawal => operation_withdraw(transaction, &mut account),
            OperationType::Dispute => operation_dispute(transaction, transactions, &mut account),
            OperationType::Resolve => operation_resolve(transaction, transactions, &mut account),
            OperationType::Chargeback => {
                operation_chargeback(transaction, transactions, &mut account)
            }
            _ => {} // Ignored
        }
    } // Operations on a locked account are ignored
}

fn operation_deposit(details: &Transaction, account: &mut AccountData) {
    if let Some(v) = details.amount {
        account.available += v;
    } // Ignored as there is nothing to do
}

fn operation_withdraw(details: &Transaction, account: &mut AccountData) {
    if let Some(v) = details.amount {
        if account.available >= v {
            account.available -= v;
        }
    } // Ignored as there is nothing to do
}

fn operation_dispute(
    details: &Transaction,
    transactions: &Transactions,
    account: &mut AccountData,
) {
    let referenced_transaction = transactions
        .iter()
        .find(|x| x.transaction_id == details.transaction_id);

    if let Some(t) = referenced_transaction {
        match t.amount {
            Some(amount)
                if details.client == t.client
                    && !account.disputes.contains(&details.transaction_id) =>
            {
                match t.operation {
                    OperationType::Deposite => {
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
}

fn operation_resolve(
    details: &Transaction,
    transactions: &Transactions,
    account: &mut AccountData,
) {
    let referenced_transaction = transactions
        .iter()
        .find(|x| x.transaction_id == details.transaction_id);

    if let Some(t) = referenced_transaction {
        match t.amount {
            Some(amount)
                if details.client == t.client
                    && account.disputes.contains(&details.transaction_id) =>
            {
                match t.operation {
                    OperationType::Deposite => {
                        account.available += amount;
                        account.held -= amount;
                    }
                    OperationType::Withdrawal => {
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
            }
            Some(_) => {} // client doesn't match or dispute doesn't exist
            None => {}    // No amount
        }
    } // No referenced transaction
}

fn operation_chargeback(
    details: &Transaction,
    transactions: &Transactions,
    account: &mut AccountData,
) {
    let referenced_transaction = transactions
        .iter()
        .find(|x| x.transaction_id == details.transaction_id);

    if let Some(t) = referenced_transaction {
        match t.amount {
            Some(amount)
                if details.client == t.client
                    && account.disputes.contains(&details.transaction_id) =>
            {
                match t.operation {
                    OperationType::Deposite => {
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
}

#[cfg(test)]
mod tests {
    use crate::engine::models::AccountData;

    use super::Transaction;
    use super::Transactions;

    #[test]
    fn two_deposites() {
        let transactions: Transactions = vec![
            Transaction {
                transaction_id: 1,
                operation: super::models::OperationType::Deposite,
                client: 10,
                amount: Some(10000),
            },
            Transaction {
                transaction_id: 2,
                operation: super::models::OperationType::Deposite,
                client: 10,
                amount: Some(10000),
            },
        ];

        let result = super::process(&transactions).unwrap();

        assert_eq!(
            &AccountData {
                available: 20000,
                held: 0,
                locked: false,
                disputes: vec![]
            },
            result.get(&10).unwrap()
        );
    }
    #[test]
    fn two_withdrawals() {
        let transactions: Transactions = vec![
            Transaction {
                transaction_id: 1,
                operation: super::models::OperationType::Withdrawal,
                client: 10,
                amount: Some(10000),
            },
            Transaction {
                transaction_id: 2,
                operation: super::models::OperationType::Withdrawal,
                client: 10,
                amount: Some(10000),
            },
        ];

        let result = super::process(&transactions).unwrap();

        assert_eq!(
            &AccountData {
                available: 0,
                held: 0,
                locked: false,
                disputes: vec![]
            },
            result.get(&10).unwrap()
        );
    }

    #[test]
    fn deposite_withdraw_balance_positive() {
        let transactions: Transactions = vec![
            Transaction {
                transaction_id: 1,
                operation: super::models::OperationType::Deposite,
                client: 10,
                amount: Some(10000),
            },
            Transaction {
                transaction_id: 2,
                operation: super::models::OperationType::Withdrawal,
                client: 10,
                amount: Some(5000),
            },
        ];

        let result = super::process(&transactions).unwrap();

        assert_eq!(
            &AccountData {
                available: 5000,
                held: 0,
                locked: false,
                disputes: vec![]
            },
            result.get(&10).unwrap()
        );
    }

    #[test]
    fn deposite_withdraw_balance_negative() {
        let transactions: Transactions = vec![
            Transaction {
                transaction_id: 1,
                operation: super::models::OperationType::Deposite,
                client: 10,
                amount: Some(10000),
            },
            Transaction {
                transaction_id: 2,
                operation: super::models::OperationType::Withdrawal,
                client: 10,
                amount: Some(15000),
            },
        ];

        let result = super::process(&transactions).unwrap();

        assert_eq!(
            &AccountData {
                available: 10000,
                held: 0,
                locked: false,
                disputes: vec![]
            },
            result.get(&10).unwrap()
        );
    }

    #[test]
    fn deposite_and_dispute() {
        let transactions: Transactions = vec![
            Transaction {
                transaction_id: 1,
                operation: super::models::OperationType::Deposite,
                client: 10,
                amount: Some(10000),
            },
            Transaction {
                transaction_id: 1,
                operation: super::models::OperationType::Dispute,
                client: 10,
                amount: None,
            },
        ];

        let result = super::process(&transactions).unwrap();

        assert_eq!(
            &AccountData {
                available: 0,
                held: 10000,
                locked: false,
                disputes: vec![1]
            },
            result.get(&10).unwrap()
        );
    }

    #[test]
    fn deposite_withdrawal_and_dispute_deposit() {
        let transactions: Transactions = vec![
            Transaction {
                transaction_id: 1,
                operation: super::models::OperationType::Deposite,
                client: 10,
                amount: Some(30000),
            },
            Transaction {
                transaction_id: 2,
                operation: super::models::OperationType::Withdrawal,
                client: 10,
                amount: Some(20000),
            },
            Transaction {
                transaction_id: 1,
                operation: super::models::OperationType::Dispute,
                client: 10,
                amount: None,
            },
        ];

        let result = super::process(&transactions).unwrap();

        assert_eq!(
            &AccountData {
                available: -20000,
                held: 30000,
                locked: false,
                disputes: vec![1]
            },
            result.get(&10).unwrap()
        );
    }

    #[test]
    fn deposite_dispute_and_resolve_deposit() {
        let transactions: Transactions = vec![
            Transaction {
                transaction_id: 1,
                operation: super::models::OperationType::Deposite,
                client: 10,
                amount: Some(10000),
            },
            Transaction {
                transaction_id: 1,
                operation: super::models::OperationType::Dispute,
                client: 10,
                amount: None,
            },
            Transaction {
                transaction_id: 1,
                operation: super::models::OperationType::Resolve,
                client: 10,
                amount: None,
            },
        ];

        let result = super::process(&transactions).unwrap();

        assert_eq!(
            &AccountData {
                available: 10000,
                held: 0,
                locked: false,
                disputes: vec![]
            },
            result.get(&10).unwrap()
        );
    }

    #[test]
    fn deposite_dispute_and_chargeback_deposit() {
        let transactions: Transactions = vec![
            Transaction {
                transaction_id: 1,
                operation: super::models::OperationType::Deposite,
                client: 10,
                amount: Some(10000),
            },
            Transaction {
                transaction_id: 1,
                operation: super::models::OperationType::Dispute,
                client: 10,
                amount: None,
            },
            Transaction {
                transaction_id: 1,
                operation: super::models::OperationType::Chargeback,
                client: 10,
                amount: None,
            },
        ];

        let result = super::process(&transactions).unwrap();

        assert_eq!(
            &AccountData {
                available: 0,
                held: 0,
                locked: true,
                disputes: vec![]
            },
            result.get(&10).unwrap()
        );
    }

    #[test]
    fn deposite_withdrawal_and_dispute_withdrawal() {
        let transactions: Transactions = vec![
            Transaction {
                transaction_id: 1,
                operation: super::models::OperationType::Deposite,
                client: 10,
                amount: Some(20000),
            },
            Transaction {
                transaction_id: 2,
                operation: super::models::OperationType::Withdrawal,
                client: 10,
                amount: Some(10000),
            },
            Transaction {
                transaction_id: 2,
                operation: super::models::OperationType::Dispute,
                client: 10,
                amount: None,
            },
        ];

        let result = super::process(&transactions).unwrap();

        assert_eq!(
            &AccountData {
                available: 10000,
                held: 10000,
                locked: false,
                disputes: vec![2]
            },
            result.get(&10).unwrap()
        );
    }

    #[test]
    fn deposite_withdrawal_dispute_and_chargeback_withdrawal() {
        let transactions: Transactions = vec![
            Transaction {
                transaction_id: 1,
                operation: super::models::OperationType::Deposite,
                client: 10,
                amount: Some(20000),
            },
            Transaction {
                transaction_id: 2,
                operation: super::models::OperationType::Withdrawal,
                client: 10,
                amount: Some(10000),
            },
            Transaction {
                transaction_id: 2,
                operation: super::models::OperationType::Dispute,
                client: 10,
                amount: None,
            },
            Transaction {
                transaction_id: 2,
                operation: super::models::OperationType::Chargeback,
                client: 10,
                amount: None,
            },
        ];

        let result = super::process(&transactions).unwrap();

        assert_eq!(
            &AccountData {
                available: 20000,
                held: 0,
                locked: true,
                disputes: vec![]
            },
            result.get(&10).unwrap()
        );
    }

    #[test]
    fn deposite_withdrawal_dispute_and_chargeback_deposit() {
        let transactions: Transactions = vec![
            Transaction {
                transaction_id: 1,
                operation: super::models::OperationType::Deposite,
                client: 10,
                amount: Some(20000),
            },
            Transaction {
                transaction_id: 2,
                operation: super::models::OperationType::Withdrawal,
                client: 10,
                amount: Some(10000),
            },
            Transaction {
                transaction_id: 1,
                operation: super::models::OperationType::Dispute,
                client: 10,
                amount: None,
            },
            Transaction {
                transaction_id: 1,
                operation: super::models::OperationType::Chargeback,
                client: 10,
                amount: None,
            },
        ];

        let result = super::process(&transactions).unwrap();

        assert_eq!(
            &AccountData {
                available: -10000,
                held: 0,
                locked: true,
                disputes: vec![]
            },
            result.get(&10).unwrap()
        );
    }

    #[test]
    fn deposite_withdrawal_dispute_and_resolve_deposit() {
        let transactions: Transactions = vec![
            Transaction {
                transaction_id: 1,
                operation: super::models::OperationType::Deposite,
                client: 10,
                amount: Some(20000),
            },
            Transaction {
                transaction_id: 2,
                operation: super::models::OperationType::Withdrawal,
                client: 10,
                amount: Some(10000),
            },
            Transaction {
                transaction_id: 1,
                operation: super::models::OperationType::Dispute,
                client: 10,
                amount: None,
            },
            Transaction {
                transaction_id: 1,
                operation: super::models::OperationType::Resolve,
                client: 10,
                amount: None,
            },
        ];

        let result = super::process(&transactions).unwrap();

        assert_eq!(
            &AccountData {
                available: 10000,
                held: 0,
                locked: false,
                disputes: vec![]
            },
            result.get(&10).unwrap()
        );
    }

    #[test]
    fn deposite_withdrawal_dispute_and_resolve_withdrawal() {
        let transactions: Transactions = vec![
            Transaction {
                transaction_id: 1,
                operation: super::models::OperationType::Deposite,
                client: 10,
                amount: Some(20000),
            },
            Transaction {
                transaction_id: 2,
                operation: super::models::OperationType::Withdrawal,
                client: 10,
                amount: Some(10000),
            },
            Transaction {
                transaction_id: 2,
                operation: super::models::OperationType::Dispute,
                client: 10,
                amount: None,
            },
            Transaction {
                transaction_id: 2,
                operation: super::models::OperationType::Resolve,
                client: 10,
                amount: None,
            },
        ];

        let result = super::process(&transactions).unwrap();

        assert_eq!(
            &AccountData {
                available: 10000,
                held: 0,
                locked: false,
                disputes: vec![]
            },
            result.get(&10).unwrap()
        );
    }

    #[test]
    fn no_deposite_on_locked_account() {
        let transactions: Transactions = vec![
            Transaction {
                transaction_id: 1,
                operation: super::models::OperationType::Deposite,
                client: 10,
                amount: Some(20000),
            },
            Transaction {
                transaction_id: 1,
                operation: super::models::OperationType::Dispute,
                client: 10,
                amount: None,
            },
            Transaction {
                transaction_id: 1,
                operation: super::models::OperationType::Chargeback,
                client: 10,
                amount: None,
            },
            Transaction {
                transaction_id: 2,
                operation: super::models::OperationType::Deposite,
                client: 10,
                amount: Some(20000),
            },
        ];

        let result = super::process(&transactions).unwrap();

        assert_eq!(
            &AccountData {
                available: 0,
                held: 0,
                locked: true,
                disputes: vec![]
            },
            result.get(&10).unwrap()
        );
    }

    #[test]
    fn no_withdrawal_on_locked_account() {
        let transactions: Transactions = vec![
            Transaction {
                transaction_id: 1,
                operation: super::models::OperationType::Deposite,
                client: 10,
                amount: Some(20000),
            },
            Transaction {
                transaction_id: 1,
                operation: super::models::OperationType::Dispute,
                client: 10,
                amount: None,
            },
            Transaction {
                transaction_id: 1,
                operation: super::models::OperationType::Chargeback,
                client: 10,
                amount: None,
            },
            Transaction {
                transaction_id: 2,
                operation: super::models::OperationType::Withdrawal,
                client: 10,
                amount: Some(20000),
            },
        ];

        let result = super::process(&transactions).unwrap();

        assert_eq!(
            &AccountData {
                available: 0,
                held: 0,
                locked: true,
                disputes: vec![]
            },
            result.get(&10).unwrap()
        );
    }
}
