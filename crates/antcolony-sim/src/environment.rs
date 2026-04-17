//! World-level runtime parameters separate from species biology.
//!
//! `Environment` holds the knobs that describe *where the colony lives* and
//! *how fast in-game time advances* — things the player picks on colony
//! creation, not things baked into the species.

use serde::{Deserialize, Serialize};

/// Real-to-in-game time multiplier. Player-selectable at colony creation.
///
/// Semantics: `in_game_seconds = multiplier × real_seconds`.
/// So at 60x, one real hour = 60 in-game hours = 2.5 in-game days.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum TimeScale {
    /// 1×: a colony literally ages as fast as you do. AntsCanada-style life-log.
    Realtime,
    /// 10×: weekly arcs. Full Camponotus first-year in ~5 real weeks.
    Brisk,
    /// 60×: see spring→summer in a real afternoon.
    Seasonal,
    /// 1440×: one real minute = one in-game day. Full colony lifespan in a weekend.
    Timelapse,
    /// Arbitrary multiplier for custom saves.
    Custom(f32),
}

impl Default for TimeScale {
    fn default() -> Self {
        TimeScale::Seasonal
    }
}

impl TimeScale {
    pub fn multiplier(&self) -> f32 {
        match self {
            TimeScale::Realtime => 1.0,
            TimeScale::Brisk => 10.0,
            TimeScale::Seasonal => 60.0,
            TimeScale::Timelapse => 1440.0,
            TimeScale::Custom(v) => v.max(0.001),
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            TimeScale::Realtime => "Realtime (1×)",
            TimeScale::Brisk => "Brisk (10×)",
            TimeScale::Seasonal => "Seasonal (60×)",
            TimeScale::Timelapse => "Timelapse (1440×)",
            TimeScale::Custom(_) => "Custom",
        }
    }
}

/// Annual climate knobs (K3). Drives ambient temperature over a 365-day
/// cycle. The sim's current day-of-year is derived from tick count and
/// `Environment.time_scale` — climate itself is scale-agnostic.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Climate {
    /// Annual-mean ambient temperature in °C.
    pub seasonal_mid_c: f32,
    /// Peak-to-mid amplitude in °C. Summer peak = mid + amp, winter trough = mid - amp.
    pub seasonal_amplitude_c: f32,
    /// Day-of-year (0..365) at which ambient reaches its summer peak.
    pub peak_day: u32,
    /// Day-of-year the simulation starts on.
    pub starting_day_of_year: u32,
}

impl Default for Climate {
    fn default() -> Self {
        Self {
            seasonal_mid_c: 15.0,
            seasonal_amplitude_c: 18.0,
            peak_day: 180,
            // Default to mid-spring so pre-K3 tests don't accidentally
            // start in sub-threshold cold. Keeper-mode sims can move this
            // to 60 (early spring) via `set_environment`.
            starting_day_of_year: 150,
        }
    }
}

/// Seasonal bucket of the year. Winter 0-78 / Spring 79-171 / Summer 172-264 /
/// Autumn 265-354 / Winter 355+.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum Season {
    Winter,
    Spring,
    Summer,
    Autumn,
}

impl Season {
    pub fn from_day_of_year(doy: u32) -> Self {
        let d = doy % 365;
        if d < 79 {
            Season::Winter
        } else if d < 172 {
            Season::Spring
        } else if d < 265 {
            Season::Summer
        } else if d < 355 {
            Season::Autumn
        } else {
            Season::Winter
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Season::Winter => "Winter",
            Season::Spring => "Spring",
            Season::Summer => "Summer",
            Season::Autumn => "Autumn",
        }
    }
}

/// Runtime environment shared by all colonies in a simulation instance.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Environment {
    pub world_width: usize,
    pub world_height: usize,
    pub time_scale: TimeScale,
    /// Simulation tick rate in Hz. This is the *smoothness* knob — higher
    /// means finer granularity per real second, NOT faster aging.
    pub tick_rate_hz: f32,
    pub seed: u64,
}

impl Default for Environment {
    fn default() -> Self {
        Self {
            world_width: 192,
            world_height: 192,
            time_scale: TimeScale::default(),
            tick_rate_hz: 30.0,
            seed: 42,
        }
    }
}

impl Environment {
    /// Convert a duration expressed in **in-game seconds** into a tick count.
    ///
    /// Derivation:
    /// - `scale = in_game_seconds_per_real_second`
    /// - `tick_rate = ticks_per_real_second`
    /// - `in_game_seconds_per_tick = scale / tick_rate`
    /// - `ticks_for_N_in_game_seconds = N / (scale / tick_rate) = N × tick_rate / scale`
    pub fn in_game_seconds_to_ticks(&self, in_game_seconds: u64) -> u32 {
        let tick_rate = self.tick_rate_hz.max(0.01);
        let scale = self.time_scale.multiplier().max(0.001);
        let t = (in_game_seconds as f64 * tick_rate as f64 / scale as f64).round();
        t.clamp(1.0, u32::MAX as f64) as u32
    }

    /// Inverse helper: ticks → in-game seconds (for UI display).
    pub fn ticks_to_in_game_seconds(&self, ticks: u64) -> u64 {
        let tick_rate = self.tick_rate_hz.max(0.01);
        let scale = self.time_scale.multiplier().max(0.001);
        let s = (ticks as f64 * scale as f64 / tick_rate as f64).round();
        s.clamp(0.0, u64::MAX as f64) as u64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn realtime_30hz_14_days_matches_expected_ticks() {
        let env = Environment {
            time_scale: TimeScale::Realtime,
            tick_rate_hz: 30.0,
            ..Environment::default()
        };
        // 14 days = 1_209_600 s; at 1× realtime, 30Hz → 14 * 86400 * 30 = 36_288_000 ticks.
        let ticks = env.in_game_seconds_to_ticks(14 * 86_400);
        assert_eq!(ticks, 36_288_000);
    }

    #[test]
    fn seasonal_60x_compresses_14_days_into_5_6_real_hours() {
        let env = Environment {
            time_scale: TimeScale::Seasonal,
            tick_rate_hz: 30.0,
            ..Environment::default()
        };
        let ticks = env.in_game_seconds_to_ticks(14 * 86_400);
        // 14 days / 60x = 5.6 real hours; at 30Hz → 604_800 ticks.
        assert_eq!(ticks, 604_800);
    }

    #[test]
    fn timelapse_1440x_14_days_is_14_real_minutes() {
        let env = Environment {
            time_scale: TimeScale::Timelapse,
            tick_rate_hz: 30.0,
            ..Environment::default()
        };
        let ticks = env.in_game_seconds_to_ticks(14 * 86_400);
        // 14 min * 60 * 30 = 25_200 ticks.
        assert_eq!(ticks, 25_200);
    }

    #[test]
    fn round_trip_seconds_ticks_seconds() {
        let env = Environment::default();
        let seconds = 3600u64;
        let t = env.in_game_seconds_to_ticks(seconds);
        let back = env.ticks_to_in_game_seconds(t as u64);
        // Allow small rounding drift.
        assert!((back as i64 - seconds as i64).abs() <= 1);
    }

    #[test]
    fn zero_in_game_seconds_still_produces_at_least_one_tick() {
        let env = Environment::default();
        assert_eq!(env.in_game_seconds_to_ticks(0), 1);
    }
}
