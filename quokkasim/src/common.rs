use std::{collections::HashMap, error::Error, fmt::{Display, Formatter, Result as FmtResult}, time::Duration};
use indexmap::IndexMap;
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

pub struct DelayMode {
    pub name: String,
    pub until_delay_distr: Distribution,
    pub until_fix_distr: Distribution,
}

pub enum DelayState {
    TimeUntilDelay(Duration),
    TimeUntilFix(Duration),
}

impl DelayState {
    pub fn as_duration(&self) -> Duration {
        match self {
            DelayState::TimeUntilDelay(duration) => *duration,
            DelayState::TimeUntilFix(duration) => *duration,
        }
    }
}

pub struct DelayStateTransition {
    pub from: Option<String>,
    pub to: Option<String>,
}

impl DelayStateTransition {
    pub fn has_changed(&self) -> bool {
        match (&self.from, &self.to) {
            (Some(a), Some(b)) => a != b,
            (None, Some(_)) => true,
            (Some(_), None) => true,
            (None, None) => false,
        }
    }
}

pub enum DelayModeChange {
    Add(DelayMode),
    Remove(String),
    RemoveAll,
}

pub struct DelayModes {
    pub modes: IndexMap<String, DelayMode>,
    pub state: IndexMap<String, DelayState>,
}

impl Default for DelayModes {
    fn default() -> Self {
        DelayModes {
            modes: IndexMap::new(),
            state: IndexMap::new(),
        }
    }
}

impl DelayModes {
    pub fn active_delay_mut(&mut self) -> Option<(&String, &mut Duration)> {
        self.state.iter_mut().find_map(|(name, state)| {
            match state {
                DelayState::TimeUntilFix(duration) => Some((name, duration)),
                _ => None,
            }
        })
    }

    pub fn active_delay(&self) -> Option<(&String, &Duration)> {
        self.state.iter().find_map(|(name, state)| {
            match state {
                DelayState::TimeUntilFix(duration) => Some((name, duration)),
                _ => None,
            }
        })
    }

    pub fn update_state(&mut self, time_elapsed: Duration) -> DelayStateTransition {
        
        // If in a delay, decrement the time remaining. If time left is zero, return the delay's name
        let active_delay = if let Some((delay_name, delay_dur_remaining)) = self.active_delay_mut() {
            *delay_dur_remaining = delay_dur_remaining.saturating_sub(time_elapsed);
            Some((delay_name.clone(), delay_dur_remaining.clone()))
        } else {
            None
        };
        let mut from: Option<String> = None;
        let mut to: Option<String> = None;

        if let Some((active_delay_name, delay_dur_remaining)) = active_delay {
            // If delay has expired, sample time until next delay
            if delay_dur_remaining.is_zero() {
                let time_until_delay_secs = self.modes.get_mut(&active_delay_name).unwrap().until_delay_distr.sample();
                let time_until_delay = Duration::from_secs_f64(time_until_delay_secs); // TODO: Units for delay, instead of assuming seconds
                self.state.insert(active_delay_name.clone(), DelayState::TimeUntilDelay(time_until_delay));
            }
            from = Some(active_delay_name.clone());
        } else {
            // If not in delay, decrement all times until delay
            self.state.iter_mut().for_each(|(name, state)| {
                match state {
                    DelayState::TimeUntilDelay(duration) => {
                        *duration = duration.saturating_sub(time_elapsed);
                    },
                    _ => {}
                }
            });
        }

        if self.active_delay().is_none() {

            // If any durations are zero, find the first and make it the active delay
            let delay_to_start = self.state.iter_mut().find_map(|(name, state)| {
                match state {
                    DelayState::TimeUntilDelay(duration) if *duration <= Duration::ZERO => {
                        Some(name.clone())
                    },
                    _ => None,
                }
            });
            if let Some(delay_to_start) = delay_to_start {
                // Sample a new time until fix for the delay
                let time_until_fix_secs = self.modes.get_mut(&delay_to_start).unwrap().until_fix_distr.sample();
                let time_until_fix = Duration::from_secs_f64(time_until_fix_secs); // TODO: Units for delay, instead of assuming seconds
                self.state.insert(delay_to_start.clone(), DelayState::TimeUntilFix(time_until_fix));
                to = Some(delay_to_start);
            }
        }
        DelayStateTransition {
            from,
            to,
        }
    }

    pub fn get_next_event(&self) -> Option<(String, DelayState)> {
        if let Some((delay_name, time_until_fix)) = self.active_delay() {
            return Some((delay_name.clone(), DelayState::TimeUntilFix(time_until_fix.clone())));
        }
        let to_next_delay = self.state.iter().filter_map(|(name, state)| {
            match state {
                DelayState::TimeUntilDelay(duration) => Some((name, duration)),
                _ => None,
            }
        }).min_by_key(|(_, duration)| *duration).map(|(name, duration)| (name.clone(), DelayState::TimeUntilDelay(duration.clone())));
        to_next_delay
    }

    /// Adds a new delay mode. The mode is instantiated assuming the delay is not currently active.
    // pub fn add_delay_mode(&mut self, name: String, mut delay_mode: DelayMode) {
    //     let time_until_delay = delay_mode.until_delay_distr.sample();
    //     self.modes.insert(name.clone(), delay_mode);
    //     self.state.insert(name, DelayState::TimeUntilDelay(Duration::from_secs_f64(time_until_delay))); // TODO: Units for delay, instead of assuming seconds
    // }

    // pub fn remove_delay_mode(&mut self, name: &String) -> Option<DelayMode> {
    //     if let Some(mode) = self.modes.shift_remove(name) {
    //         self.state.shift_remove(name);
    //         Some(mode)
    //     } else {
    //         None
    //     }
    // }
    pub fn modify(&mut self, change: DelayModeChange) {
        match change {
            DelayModeChange::Add(mut mode) => {
                let time_until_delay = mode.until_delay_distr.sample();
                let delay_name = mode.name.clone();
                self.modes.insert(delay_name.clone(), mode);
                self.state.insert(delay_name, DelayState::TimeUntilDelay(Duration::from_secs_f64(time_until_delay))); // TODO: Units for delay, instead of assuming seconds
            },
            DelayModeChange::Remove(name) => {
                self.modes.shift_remove(&name);
                self.state.shift_remove(&name);
            },
            DelayModeChange::RemoveAll => {
                self.modes.clear();
                self.state.clear();
            }
        }
    }
}
