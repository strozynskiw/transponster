use anyhow::Result;
use csv::{ReaderBuilder, Trim};
use fs::File;
use serde::Deserialize;
use std::{env, fs, process::exit};

mod engine;
use engine::models::{OperationType, Transaction};

#[derive(Debug, Deserialize)]

pub struct TransactionInput {
    #[serde(rename = "type")]
    pub transaction: String,
    pub client: u16,
    #[serde(rename = "tx")]
    pub id: u32,
    pub amount: Option<String>,
}

// Yes. I know... But I have tests for this!
fn string_to_money(input: &str) -> i64 {
    let parts: Vec<&str> = input.split('.').collect();
    let mut amount: i64 = (parts[0].parse::<i64>().unwrap_or_default() * 10000) as i64;
    if parts.len() == 2 {
        amount += format!("{:0<4}", parts[1])
            .chars()
            .take(4)
            .collect::<String>()
            .parse::<i64>()
            .unwrap_or_default() as i64;
    }
    amount
}

fn money_to_string(input: &i64) -> String {
    let integral = input / 10000;
    let decimal = input - integral * 10000;

    format!("{}.{:04}", integral, decimal)
}

fn parse(path: &str) -> Result<Vec<Transaction>> {
    let mut output: Vec<engine::models::Transaction> = Vec::new();
    let file = File::open(path)?;
    let mut rdr = ReaderBuilder::new().trim(Trim::All).from_reader(file);
    for result in rdr.deserialize() {
        let record: TransactionInput = result?;

        output.push(Transaction {
            transaction_id: record.id,
            operation: match record.transaction.to_ascii_lowercase().as_str() {
                "deposit" => OperationType::Deposite,
                "withdrawal" => OperationType::Withdrawal,
                "dispute" => OperationType::Dispute,
                "resolve" => OperationType::Resolve,
                "chargeback" => OperationType::Chargeback,
                _ => OperationType::Unknown,
            },
            client: record.client,
            amount: record.amount.map(|value| string_to_money(&value)),
        });
    }

    Ok(output)
}

fn print_map(result: &engine::AccountsMap) -> Result<()> {
    print!("client,available,held,total,locked");

    result.iter().for_each(|(c, d)| {
        print!(
            "\n{},{},{},{},{}",
            c,
            money_to_string(&d.available),
            money_to_string(&d.held),
            money_to_string(&(d.available + d.held)),
            d.locked
        )
    });

    Ok(())
}

fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();

    if args.len() != 2 {
        println!("Provide exactly one argument!");
        exit(-1);
    }

    let output = engine::process(&parse(&args[1])?)?;

    print_map(&output)?;

    Ok(())
}

#[test]
fn test_money_to_string() {
    assert_eq!(money_to_string(&1), "0.0001");
    assert_eq!(money_to_string(&10), "0.0010");
    assert_eq!(money_to_string(&100), "0.0100");
    assert_eq!(money_to_string(&2000), "0.2000");
    assert_eq!(money_to_string(&22000), "2.2000");
    assert_eq!(money_to_string(&220000), "22.0000");
    assert_eq!(money_to_string(&2200000), "220.0000");
    assert_eq!(money_to_string(&22000000), "2200.0000");
    assert_eq!(money_to_string(&220000000), "22000.0000");
    assert_eq!(money_to_string(&22000000000), "2200000.0000");
    assert_eq!(money_to_string(&2200000000001), "220000000.0001");
    assert_eq!(money_to_string(&220000000000001), "22000000000.0001");
    assert_eq!(money_to_string(&2200000000000001), "220000000000.0001");
    assert_eq!(money_to_string(&22000000000000001), "2200000000000.0001");
    assert_eq!(
        money_to_string(&2200000000000000001),
        "220000000000000.0001"
    );
}

#[test]
fn test_string_to_money() {
    assert_eq!(string_to_money("0.00001"), 0);
    assert_eq!(string_to_money("0.00009"), 0);
    assert_eq!(string_to_money("0.0001"), 1);
    assert_eq!(string_to_money("0.0011"), 11);
    assert_eq!(string_to_money("0.0101"), 101);
    assert_eq!(string_to_money("4.0001"), 40001);
    assert_eq!(string_to_money("40.0002"), 400002);
    assert_eq!(string_to_money("400.0002"), 4000002);
    assert_eq!(string_to_money("400.4303"), 4004303);
    assert_eq!(string_to_money("22000000000000.0001"), 220000000000000001);
    assert_eq!(string_to_money("220000000000000.0001"), 2200000000000000001);
    assert_eq!(string_to_money("1"), 10000);
    assert_eq!(string_to_money("1.0"), 10000);
    assert_eq!(string_to_money("1.50"), 15000);
    assert_eq!(string_to_money("1.5"), 15000);
    assert_eq!(string_to_money("1.05"), 10500);
    assert_eq!(string_to_money("1."), 10000);
    assert_eq!(string_to_money(".10"), 1000);
}
