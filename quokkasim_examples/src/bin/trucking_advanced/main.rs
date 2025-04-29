mod model_construction;
mod simulation;
mod components;
mod loggers;

use std::{fs::File, io::BufReader};

use clap::Parser;
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use simulation::build_and_run_model;
use model_construction::ModelConfig;

/// Trucking simulation command line options.
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct CLIArgs {
    /// The base seed used for random distributions.
    #[arg(long, default_value = "1")]
    pub seed: String,

    /// The number of trucks to simulate.
    #[arg(long, default_value = "2")]
    pub num_trucks: usize,

    /// The simulation duration in seconds.
    #[arg(long, default_value = "21600")]
    pub sim_duration_secs: f64,

    /// Config file path
    #[arg(long, default_value = "quokkasim_examples/src/bin/trucking_advanced/model_config.yaml")]
    pub config_file: String,
}

pub struct ParsedArgs {
    pub seed: u64,
    pub num_trucks: usize,
    pub sim_duration_secs: f64,
}

/// Parse a seed string such as "0..6" or "0..=7" into a vector of u64 values.
pub fn parse_seed_range(seed_str: &str) -> Result<Vec<u64>, String> {
    if let Some(idx) = seed_str.find("..=") {
        let (start, end) = seed_str.split_at(idx);
        let end = &end[3..]; // skip "..="
        let start: u64 = start
            .trim()
            .parse()
            .map_err(|e| format!("Invalid start seed '{}': {}", start.trim(), e))?;
        let end: u64 = end
            .trim()
            .parse()
            .map_err(|e| format!("Invalid end seed '{}': {}", end.trim(), e))?;
        Ok((start..=end).collect())
    } else if let Some(idx) = seed_str.find("..") {
        let (start, end) = seed_str.split_at(idx);
        let end = &end[2..]; // skip ".."
        let start: u64 = start
            .trim()
            .parse()
            .map_err(|e| format!("Invalid start seed '{}': {}", start.trim(), e))?;
        let end: u64 = end
            .trim()
            .parse()
            .map_err(|e| format!("Invalid end seed '{}': {}", end.trim(), e))?;
        Ok((start..end).collect())
    } else {
        seed_str
            .trim()
            .parse::<u64>()
            .map(|v| vec![v])
            .map_err(|e| format!("Invalid seed value '{}': {}", seed_str, e))
    }
}

fn main() {
    let args = CLIArgs::parse();

    let seeds = match parse_seed_range(&args.seed) {
        Ok(seeds) => seeds,
        Err(err) => {
            eprintln!("Error parsing seed range: {}", err);
            std::process::exit(1);
        }
    };

    let file = match File::open(&args.config_file) {
        Ok(file) => file,
        Err(err) => {
            eprintln!("Error opening model config file {}: {}", args.config_file, err);
            std::process::exit(1);
        }
    };
    let reader = BufReader::new(file);
    let config: ModelConfig = serde_yaml::from_reader(reader).unwrap();

    seeds.par_iter().for_each(|seed| {
        let args = ParsedArgs {
            seed: *seed,
            num_trucks: args.num_trucks,
            sim_duration_secs: args.sim_duration_secs,
        };
        build_and_run_model(args, config.clone());
    });
}
