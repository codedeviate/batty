mod cli;
mod config;
mod highlight;
mod input;
mod syntax;

use clap::Parser;
use cli::Cli;

fn main() {
    let mut all_args: Vec<String> = std::env::args().take(1).collect(); // program name
    all_args.extend(config::load_args());
    all_args.extend(std::env::args().skip(1));
    let args = Cli::parse_from(all_args);
    eprintln!("parsed: {:?}", args);
}
