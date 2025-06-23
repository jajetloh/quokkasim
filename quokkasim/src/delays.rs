use std::time::Duration;
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use crate::common::Distribution;

#[derive(Debug, Clone)]
pub struct DelayMode {
    pub name: String,
    pub until_delay_distr: Distribution,
    pub until_fix_distr: Distribution,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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

#[derive(Debug, Clone)]
pub enum DelayModeChange {
    Add(DelayMode),
    Remove(String),
    RemoveAll,
}

#[derive(Debug, Clone)]
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
            Some((delay_name.clone(), *delay_dur_remaining))
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
                if let DelayState::TimeUntilDelay(duration) = state {
                    *duration = duration.saturating_sub(time_elapsed);
                }
            });
        }

        let active_delay = self.active_delay();
        if let Some((active_delay_name, _)) = active_delay {
            // If still in delay, return the name of the delay
            to = Some(active_delay_name.clone());
        } else {
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
            return Some((delay_name.clone(), DelayState::TimeUntilFix(*time_until_fix)));
        }
        let to_next_delay = self.state.iter().filter_map(|(name, state)| {
            match state {
                DelayState::TimeUntilDelay(duration) => Some((name, duration)),
                _ => None,
            }
        }).min_by_key(|(_, duration)| *duration).map(|(name, duration)| (name.clone(), DelayState::TimeUntilDelay(*duration)));
        to_next_delay
    }

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


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_transition_for_single_delay() {
        let mut dm = DelayModes::default();
        dm.modify(DelayModeChange::Add(DelayMode {
            name: "TestDelay".to_string(),
            until_delay_distr: Distribution::Constant(13.0),
            until_fix_distr: Distribution::Constant(5.0),
        }));

        let update_1 = dm.update_state(Duration::from_secs(4));
        assert_eq!(update_1, DelayStateTransition { from: None, to: None });

        let update_2 = dm.update_state(Duration::from_secs(10));
        assert_eq!(update_2, DelayStateTransition { from: None, to: Some("TestDelay".to_string()) });

        let update_3 = dm.update_state(Duration::from_secs(1));
        assert_eq!(update_3, DelayStateTransition { from: Some("TestDelay".to_string()), to: Some("TestDelay".to_string()) });

        let update_4 = dm.update_state(Duration::from_secs(5));
        assert_eq!(update_4, DelayStateTransition { from: Some("TestDelay".to_string()), to: None });
    }

    #[test]
    fn test_transition_for_multiple_delays_and_time_to_next() {
        let mut dm = DelayModes::default();
        dm.modify(DelayModeChange::Add(DelayMode {
            name: "Delays1".to_string(),
            until_delay_distr: Distribution::Constant(13.0),
            until_fix_distr: Distribution::Constant(5.0),
        }));
        dm.modify(DelayModeChange::Add(DelayMode {
            name: "Delays2".to_string(),
            until_delay_distr: Distribution::Constant(15.0),
            until_fix_distr: Distribution::Constant(4.0),
        }));

        let duration_to_next = dm.get_next_event().unwrap().1.as_duration();
        assert_eq!(duration_to_next, Duration::from_secs(13));
        let update_1 = dm.update_state(duration_to_next);
        assert_eq!(update_1, DelayStateTransition { from: None, to: Some("Delays1".to_string()) });

        let duration_to_next = dm.get_next_event().unwrap().1.as_duration();
        assert_eq!(duration_to_next, Duration::from_secs(5));
        let update_2 = dm.update_state(duration_to_next);
        assert_eq!(update_2, DelayStateTransition { from: Some("Delays1".to_string()), to: None });

        let duration_to_next = dm.get_next_event().unwrap().1.as_duration();
        assert_eq!(duration_to_next, Duration::from_secs(15 - 13));
        let update_3 = dm.update_state(duration_to_next);
        assert_eq!(update_3, DelayStateTransition { from: None, to: Some("Delays2".to_string()) });

        let duration_to_next = dm.get_next_event().unwrap().1.as_duration();
        assert_eq!(duration_to_next, Duration::from_secs(4));
        let update_4 = dm.update_state(duration_to_next);
        assert_eq!(update_4, DelayStateTransition { from: Some("Delays2".to_string()), to: None });

        let duration_to_next = dm.get_next_event().unwrap().1.as_duration();
        assert_eq!(duration_to_next, Duration::from_secs(11));
        let update_5 = dm.update_state(duration_to_next);
        assert_eq!(update_5, DelayStateTransition { from: None, to: Some("Delays1".to_string()) });
    }
}