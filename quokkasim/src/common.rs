use std::{error::Error, fmt::{Display, Formatter, Result as FmtResult}};
use rand::{rngs::SmallRng, SeedableRng};
use rand_distr::{Distribution as _, Exp, Normal, Triangular, Uniform};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize)]
/// A short, lightweight identifier for an event. Very useful for understanding causal flow of events via log files.
/// Conventionally of the form `PROC_123456`, with a prefix uniquely identifying the process, and suffix an auto-incrementing number.
pub struct EventId(pub String);

impl EventId {
    pub fn from_init() -> EventId {
        EventId("INIT_000000".to_string())
    }

    pub fn from_scheduler() -> EventId {
        EventId("SCH_000000".to_string())
    }
}

#[derive(Debug, Clone)]
/// An instantiated Distribution that can be sampled from via the `sample` method.
/// Usually constructed via the `DistributionFactory::create` method, though the Constant variant can be constructed directly.
pub enum Distribution {
    Uniform(Uniform<f64>, SmallRng),
    Triangular(Triangular<f64>, SmallRng),
    Constant(f64),
    Normal(Normal<f64>, SmallRng),
    TruncNormal { normal_dist: Normal<f64>, min: f64, max: f64, rng: SmallRng },
    Exponential(Exp<f64>, SmallRng),
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "type")]
/// Serialisable configuration for creating a Distribution instance. Uniquely defines the distribution to be created, excluding the random number generator.
pub enum DistributionConfig {
    Uniform { min: f64, max: f64},
    Triangular { min: f64, max: f64, mode: f64 },
    Constant(f64),
    Normal { mean: f64, std: f64 },
    TruncNormal { mean: f64, std: f64, min: Option<f64>, max: Option<f64> },
    Exponential { mean: f64 },
}

/// Factory for creating Distribution instances based on a DistributionConfig. For random distributions, creates SmallRng instances seeded with an incrementing seed value.
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
    pub fn new(base_seed: u64) -> Self {
        DistributionFactory {
            base_seed,
            next_seed: base_seed,
        }
    }

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
            DistributionConfig::TruncNormal { mean, std, min, max } => {
                match Normal::new(mean, std) {
                    Ok(dist) => {

                        let min = min.unwrap_or(f64::MIN);
                        let max = max.unwrap_or(f64::MAX);

                        if min >= max {
                            return Err(DistributionParametersError {
                                msg: "Minimum value cannot be greater than or equal maximum value".to_string()
                            })
                        }

                        let rng = SmallRng::seed_from_u64(self.next_seed);
                        return Ok(Distribution::TruncNormal { normal_dist: dist, min, max, rng })
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
            Distribution::TruncNormal { normal_dist, min, max, rng } => {
                loop {
                    let x = normal_dist.sample(rng);
                    if x >= *min && x <= *max {
                        break x;
                    }
                }
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