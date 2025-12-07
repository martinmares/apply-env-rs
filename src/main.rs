use std::env;
use std::process;

use apply_env::{parse_args, print_version, run_from_stdio, CliResult, HELP_TEXT};

fn main() {
    // přeskočíme jméno programu
    let args: Vec<String> = env::args().skip(1).collect();

    match parse_args(args) {
        CliResult::Run(cfg) => {
            if let Err(err) = run_from_stdio(cfg) {
                eprintln!("ERROR: {}", err);
                process::exit(1);
            }
        }
        CliResult::Help => {
            println!("{}", HELP_TEXT);
        }
        CliResult::Version => {
            print_version();
        }
        CliResult::InvalidOption(flag) => {
            eprintln!("ERROR: {} is not a valid option.", flag);
            eprintln!("{}", HELP_TEXT);
            process::exit(1);
        }
    }
}

