mod utils;
mod simulation;
mod components;

use std::{fs::File, io::BufReader};

use clap::Parser;
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use simulation::build_and_run_model;
use utils::{parse_seed_range, CLIArgs, ModelConfig};
pub use utils::ParsedArgs;

fn main() {
    let args = CLIArgs::parse();

    let seeds = match parse_seed_range(&args.seed) {
        Ok(seeds) => seeds,
        Err(err) => {
            eprintln!("Error parsing seed range: {}", err);
            std::process::exit(1);
        }
    };

    let file = File::open("quokkasim/src/my_config_2.yaml").unwrap();
    let reader = BufReader::new(file);
    let config: ModelConfig = serde_yaml::from_reader(reader).unwrap();

    // println!("{:#?}", config);

    seeds.par_iter().for_each(|seed| {
        let args = ParsedArgs {
            seed: *seed,
            num_trucks: args.num_trucks,
            sim_duration_secs: args.sim_duration_secs,
        };
        build_and_run_model(args, config.clone());
    });
}
