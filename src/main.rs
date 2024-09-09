use anyhow::Result;
use std::path::PathBuf;

mod engine;
use engine::Engine;

use structopt::StructOpt;

#[derive(Debug, StructOpt)]
struct Args {
    #[structopt(parse(from_os_str))]
    input: PathBuf,
}

fn main() -> Result<()> {
    // It's probably too much but it provides nice guides
    let args = Args::from_args_safe()?;

    let mut engine = Engine::new();

    engine.process_input(&args.input)?;

    engine.serialize_report_stdout()?;

    Ok(())
}

// Some high level integration tests
// Mostly checking parsing and output format
#[cfg(test)]
mod tests {
    use std::io::BufWriter;

    use csv::{ReaderBuilder, Trim, Writer};

    use crate::engine::Engine;

    #[test]
    fn simple_input() {
        let input = "\
        type, client, tx, amount
        deposit, 1, 1, 1.0
        deposit, 2, 2, 2.0
        deposit, 1, 3, 2.0
        withdrawal, 1, 4, 1.5
        withdrawal, 2, 5, 3.0";

        let result = run_test(input);

        assert_eq!(
            result,
            "client,available,held,total,locked\n1,1.5,0,1.5,false\n2,2,0,2,false\n"
        );
    }

    #[test]
    fn more_complex_input() {
        let input = "\
            type, client, tx, amount
            deposit, 1, 1, 1.0
            deposit, 1, 2, 2.0
            dispute, 1, 1
            chargeback, 1, 1
            withdrawal, 1, 3, 1
            deposit, 1, 3, 2.0";

        let result = run_test(input);

        assert_eq!(result, "client,available,held,total,locked\n1,2,0,2,true\n");
    }

    fn run_test(input: &str) -> String {
        let reader = ReaderBuilder::new()
            .flexible(true)
            .trim(Trim::All)
            .from_reader(input.as_bytes());

        let mut engine = Engine::new();
        engine.process_from_reader(reader).unwrap();

        let buffer = Vec::new();
        let mut buf_writer = BufWriter::new(buffer);
        let writer = Writer::from_writer(&mut buf_writer);

        engine.serialize_report_to_writer(writer).unwrap();

        String::from_utf8_lossy(&buf_writer.into_inner().unwrap()).into_owned()
    }
}
