use anyhow::Result;
use engine::Engine;
use std::{env, path::Path, process::exit};

mod engine;

fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();

    if args.len() != 2 {
        println!("Provide exactly one argument!");
        exit(-1);
    }

    let file_path = Path::new(&args[1]);

    let mut engine = Engine::new();

    engine.process_input(file_path)?;

    engine.print_report();

    //print_map(&output)?;

    Ok(())
}
