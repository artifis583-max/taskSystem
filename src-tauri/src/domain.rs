//! Pure game rules. No I/O, no clock — everything is passed in.

use crate::config;
use chrono::{Datelike, NaiveDate};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Size {
    Small,
    Medium,
    Large,
}

impl Size {
    pub fn base(self) -> (i64, i64) {
        match self {
            Size::Small => config::SMALL,
            Size::Medium => config::MEDIUM,
            Size::Large => config::LARGE,
        }
    }
    pub fn as_str(self) -> &'static str {
        match self {
            Size::Small => "small",
            Size::Medium => "medium",
            Size::Large => "large",
        }
    }
    pub fn parse(s: &str) -> Option<Size> {
        match s {
            "small" => Some(Size::Small),
            "medium" => Some(Size::Medium),
            "large" => Some(Size::Large),
            _ => None,
        }
    }
}

/// How a task repeats. Stored as JSON on the task row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Schedule {
    Once,
    Daily,
    /// 0 = Monday … 6 = Sunday.
    Weekdays { days: Vec<u8> },
    EveryNDays { n: u32 },
}

impl Schedule {
    pub fn is_valid(&self) -> bool {
        match self {
            Schedule::Weekdays { days } => !days.is_empty() && days.iter().all(|d| *d < 7),
            Schedule::EveryNDays { n } => (1..=365).contains(n),
            _ => true,
        }
    }

    /// Does a task with this schedule have an occurrence on `date`?
    pub fn occurs_on(&self, start: NaiveDate, end: Option<NaiveDate>, date: NaiveDate) -> bool {
        if date < start || end.is_some_and(|e| date > e) {
            return false;
        }
        match self {
            Schedule::Once => date == start,
            Schedule::Daily => true,
            Schedule::Weekdays { days } => days.contains(&(date.weekday().num_days_from_monday() as u8)),
            Schedule::EveryNDays { n } => (date - start).num_days() % (*n as i64) == 0,
        }
    }
}

/// Coins for completing a task: level bonus first, then rollover penalty, then round, min 1.
pub fn coins_for(base: i64, level: i64, rollover_count: i64) -> i64 {
    if base <= 0 {
        return 0;
    }
    let level_adjusted = base as f64 * config::LEVEL_MULTIPLIER.powi((level - 1).max(0) as i32);
    let after_rollover = level_adjusted * config::ROLLOVER_FACTOR.powi(rollover_count.max(0) as i32);
    (after_rollover.round() as i64).max(1)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum DayResult {
    Qualified,
    Neutral,
    Broken,
}

impl DayResult {
    pub fn as_str(self) -> &'static str {
        match self {
            DayResult::Qualified => "qualified",
            DayResult::Neutral => "neutral",
            DayResult::Broken => "broken",
        }
    }
}

pub fn day_result(eligible: i64, completed: i64) -> DayResult {
    if eligible <= 0 {
        DayResult::Neutral
    } else if completed * 100 >= eligible * config::QUALIFY_PERCENT {
        DayResult::Qualified
    } else {
        DayResult::Broken
    }
}

pub fn next_streak(prev: i64, result: DayResult) -> i64 {
    match result {
        DayResult::Qualified => prev + 1,
        DayResult::Neutral => prev,
        DayResult::Broken => 0,
    }
}

/// Highest level whose XP and streak requirements are both met.
pub fn level_for(xp: i64, streak: i64) -> i64 {
    config::LEVELS
        .iter()
        .filter(|(_, min_xp, min_streak)| xp >= *min_xp && streak >= *min_streak)
        .map(|(lvl, _, _)| *lvl)
        .max()
        .unwrap_or(1)
}

/// Requirements of the level after `level`, if any: (level, xp, streak).
pub fn next_level(level: i64) -> Option<(i64, i64, i64)> {
    config::LEVELS.iter().copied().find(|(l, _, _)| *l == level + 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(s: &str) -> NaiveDate {
        NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
    }

    #[test]
    fn coins_same_day_level_1() {
        assert_eq!(coins_for(10, 1, 0), 10);
    }

    #[test]
    fn coins_level_bonus() {
        // round(20 × 1.15²) = round(26.45)
        assert_eq!(coins_for(20, 3, 0), 26);
    }

    #[test]
    fn coins_rollover_compounds_from_original() {
        // round(20 × 0.8²) = round(12.8)
        assert_eq!(coins_for(20, 1, 2), 13);
        assert_eq!(coins_for(20, 1, 1), 16);
    }

    #[test]
    fn coins_never_below_one() {
        assert_eq!(coins_for(10, 1, 30), 1);
        assert_eq!(coins_for(0, 1, 0), 0);
    }

    #[test]
    fn coins_level_then_rollover() {
        // round(30 × 1.15 × 0.8) = round(27.6)
        assert_eq!(coins_for(30, 2, 1), 28);
    }

    #[test]
    fn streak_threshold() {
        assert_eq!(day_result(2, 1), DayResult::Qualified); // 50%
        assert_eq!(day_result(100, 49), DayResult::Broken); // 49%
        assert_eq!(day_result(100, 50), DayResult::Qualified);
        assert_eq!(day_result(1, 0), DayResult::Broken);
        assert_eq!(day_result(0, 0), DayResult::Neutral);
    }

    #[test]
    fn streak_progression() {
        assert_eq!(next_streak(4, DayResult::Qualified), 5);
        assert_eq!(next_streak(4, DayResult::Neutral), 4);
        assert_eq!(next_streak(4, DayResult::Broken), 0);
    }

    #[test]
    fn level_needs_both_xp_and_streak() {
        assert_eq!(level_for(1_500, 5), 2);
        assert_eq!(level_for(100, 40), 2);
        assert_eq!(level_for(99, 40), 1);
        assert_eq!(level_for(6_000, 100), 7);
        assert_eq!(level_for(10_000, 0), 1);
    }

    #[test]
    fn schedules() {
        let start = d("2026-10-19"); // Monday
        assert!(Schedule::Once.occurs_on(start, None, start));
        assert!(!Schedule::Once.occurs_on(start, None, d("2026-10-20")));
        assert!(Schedule::Daily.occurs_on(start, None, d("2026-10-25")));
        assert!(!Schedule::Daily.occurs_on(start, Some(d("2026-10-22")), d("2026-10-23")));
        assert!(!Schedule::Daily.occurs_on(start, None, d("2026-10-18")));
        let mwf = Schedule::Weekdays { days: vec![0, 2, 4] };
        assert!(mwf.occurs_on(start, None, d("2026-10-21"))); // Wed
        assert!(!mwf.occurs_on(start, None, d("2026-10-22"))); // Thu
        let every3 = Schedule::EveryNDays { n: 3 };
        assert!(every3.occurs_on(start, None, d("2026-10-22")));
        assert!(!every3.occurs_on(start, None, d("2026-10-23")));
    }

    #[test]
    fn schedule_validation() {
        assert!(!Schedule::Weekdays { days: vec![] }.is_valid());
        assert!(!Schedule::Weekdays { days: vec![7] }.is_valid());
        assert!(!Schedule::EveryNDays { n: 0 }.is_valid());
        assert!(Schedule::EveryNDays { n: 2 }.is_valid());
    }
}
