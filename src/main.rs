use clap::Parser;
use code_search::error::Result;
use code_search::Cli;

fn main() -> Result<()> {
    let cli = Cli::parse();
    code_search::run(cli)
}
