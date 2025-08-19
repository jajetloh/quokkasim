use std::path::PathBuf;
use clap::{Parser, Subcommand};
use anyhow::Result;
use quokkasim::inject_boilerplate; // assume you expose this from your lib

/// cargo-foo — inject my_crate’s boilerplate into your project
#[derive(Parser)]
#[command(author, version, about)]
struct Opt {
    #[command(subcommand)]
    cmd: Command,
}

#[derive(Subcommand)]
enum Command {
    /// inject boilerplate into a file
    Inject {
        /// file to modify
        #[arg(value_name = "FILE")]
        file: PathBuf,
    },
}

fn main() -> Result<()> {
    let opt = Opt::parse();
    match opt.cmd {
        Command::Inject { file } => inject_boilerplate(&file),
    }
}