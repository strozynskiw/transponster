
# Transponster

This is an implementation of a simple transaction processor that takes CSV.

## Usage

The engine takes a csv input from file and then puts the result to stdout (and error logs to stderr).

To run the engine type:
```sh
cargo run -- input.csv > output.csv
```

## Design decisions

### Main
- Used `Structopt` for argument parsing, to minimize the amount of code to read. It's probably unnecessary in that simple case but since it's an app dependency, and not engine dependency, it should be safe.
- Main only does basic operations hence engine only takes input and output path and does all the necessary operations returning only those errors, that causes the parsing and processing to fail.


### Engine
- Use `Decimal` for keeping the money amount - this is much more precise than floating points (forbidden here), and much more convenient than keeping the value as just integer (value * 1000).
- Engine only returns errors that causes the whole operation to fail (missing file for example). Otherwise returned error is just printed to stderr so transaction parsing is uninterrupted
- I used checked operations from `Decimal` - As far as I know it's 128 bit value, so there is very little chance for it to overflow but better be safe then sorry.
- ProcessingErrors are always printed to stderr - since we only read stdout it should not be a problem.
- Only withdraw and deposit transactions are stored in the transaction list for particular account. That should optimize runtime performance.
- Simple interface for loading file, and serializing output to stdout was provided as reader/writer interface so the input could be provided from elsewhere. It is especially useful in integration tests where input and output are just strings.
- I used `indexmap` so the output of engine is consistent without sorting.


## Assumptions/Comments
- If client id from withdraw/dispute/resolve is different than the on in the referenced transaction, the transaction is ignored (Error MissingTransaction is returned).
- An account can reach negative balance if a user withdrawn money after an incorrect deposit. Account will be then locked with negative balance.
- When withdrawal is disputed, the disputed amount is added to held value. In this case total founds increases (while it remain the same when a deposit is disputed - as it suppose to according to the paper). Then resolution moves amount from held to available (withdraw indeed did not happen), or is charged back in case money was actually withdrawn and the dispute is false.
- Output precision will be the same as assumed input precision in case of `Decimal`.
- Negative amounts are ignored (return error to stderr).
- Transaction ids are expected to be globally unique.
- Dispute/Release/Chargeback transactions must contain correct client id.
- Locked accounts can not be further disputed and released as well.


## Testing

### Unit tests
- Engine logic us tested by it's unit tests. Not everything is covered, but the main logic and most common cases are covered by the unit tests. Ful coverage would require another a couple of hours of work probably.

### Integration tests
- Full flow tests are placed in main.rs and simulate integration tests with real input and output.


## Improvements
- Better code coverage.
- There still is some duplication that could be removed.
- Operations could be part of AccountData implementation since all of them take account data as first parameter.


## Security
There is just one warning reported from `cargo audit` and it's related to `proc-macro-error` that is now pronounced unmaintained but not actually dangerous.