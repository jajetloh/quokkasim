use std::{error::Error, fmt::{Display, Formatter, Result as FmtResult}};

use nexosim::{ports::EventBuffer, time::MonotonicTime};
use rand::{rngs::SmallRng, SeedableRng};
use rand_distr::{Distribution as _, Exp, Normal, Triangular, Uniform};

#[derive(Debug, Clone)]
pub struct EventLog {
    pub time: MonotonicTime,
    pub element_name: String,
    pub element_type: String,
    pub log_type: String,
    pub json_data: String,
}

#[derive()]
pub struct EventLogger {
    pub buffer: EventBuffer<EventLog>
}

impl EventLogger {
    pub fn new(capacity: usize) -> Self {
        EventLogger {
            buffer: EventBuffer::with_capacity(capacity)
        }
    }

    pub fn write_csv(self, filename: &str) -> Result<(), std::io::Error> {
        let file = std::fs::File::create(filename)?;
        let mut writer = csv::Writer::from_writer(file);
        self.buffer.for_each(|log| {
            let record = [
                log.time.as_secs().to_string(),
                log.time.subsec_nanos().to_string(),
                log.element_name.clone(),
                log.element_type.clone(),
                log.log_type.clone(),
                log.json_data.clone()
            ];
            writer.write_record(&record).expect("Failed to write record");
        });
        writer.flush().expect("Failed to flush writer");
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct NotificationMetadata {
    pub time: MonotonicTime,
    pub element_from: String,
    pub message: String,
}


pub enum Distribution {
    Uniform(Uniform<f64>, SmallRng),
    Triangular(Triangular<f64>, SmallRng),
    Constant(f64),
    Normal(Normal<f64>, SmallRng),
    Exponential(Exp<f64>, SmallRng),
}

pub enum DistributionConfig {
    Uniform { min: f64, max: f64},
    Triangular { min: f64, max: f64, mode: f64 },
    Constant(f64),
    Normal { mean: f64, std: f64 },
    Exponential { mean: f64 },
}

pub struct DistributionFactory {
    pub base_seed: u64,
    pub next_seed: u64,
}

#[derive(Debug)]
pub struct DistributionParametersError {
    pub msg: String
}

impl Error for DistributionParametersError {}

impl Display for DistributionParametersError {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(f, "{}", self.msg)
    }
}

impl DistributionFactory {
    pub fn create(&mut self, config: DistributionConfig) -> Result<Distribution, DistributionParametersError> {
        let result = match config {
            DistributionConfig::Uniform { min, max } => {
                let rng = SmallRng::seed_from_u64(self.next_seed);
                Ok(Distribution::Uniform(Uniform::new(min, max), rng))
            },
            DistributionConfig::Triangular { min, max, mode } => {
                let triangle_dist = Triangular::new(min, max, mode);
                match triangle_dist {
                    Ok(dist) => {
                        let rng = SmallRng::seed_from_u64(self.next_seed);
                        Ok(Distribution::Triangular(dist, rng))
                    },
                    Err(e) => {
                        Err(DistributionParametersError {
                            msg: e.to_string()
                        })
                    }
                }
            },
            DistributionConfig::Constant(x) => Ok(Distribution::Constant(x)),
            DistributionConfig::Normal { mean , std } => {
                match Normal::new(mean, std) {
                    Ok(dist) => {
                        let rng = SmallRng::seed_from_u64(self.next_seed);
                        return Ok(Distribution::Normal(dist, rng))
                    },
                    Err(e) => {
                        return Err(DistributionParametersError {
                            msg: e.to_string()
                        })
                    }
                }
            },
            DistributionConfig::Exponential { mean } => {
                match Exp::new(1. / mean) {
                    Ok(dist) => {
                        let rng = SmallRng::seed_from_u64(self.next_seed);
                        return Ok(Distribution::Exponential(dist, rng))
                    },
                    Err(e) => {
                        return Err(DistributionParametersError {
                            msg: e.to_string()
                        })
                    }
                }
            }
        };

        self.next_seed += 1;

        result
    }
}

impl Distribution {
    pub fn sample(&mut self) -> f64 {
        match self {
            Distribution::Uniform(dist, rng) => {
                dist.sample(rng)
            },
            Distribution::Triangular(dist, rng) => {
                dist.sample(rng)
            },
            Distribution::Constant(value) => {
                *value
            },
            Distribution::Normal(dist, rng) => {
                dist.sample(rng)
            },
            Distribution::Exponential(dist, rng) => {
                dist.sample(rng)
            }
        }
    }
}

impl Default for Distribution {
    fn default() -> Self {
        Distribution::Constant(1.)
    }
}