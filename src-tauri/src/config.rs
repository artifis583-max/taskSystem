//! Every tunable number in the app lives here. Nothing else hard-codes these.

/// (coins, xp) per task size.
pub const SMALL: (i64, i64) = (10, 10);
pub const MEDIUM: (i64, i64) = (20, 20);
pub const LARGE: (i64, i64) = (30, 30);

/// Each rollover multiplies the coin reward by this (compounding).
pub const ROLLOVER_FACTOR: f64 = 0.8;

/// Coin bonus per level above 1: base × LEVEL_MULTIPLIER^(level − 1).
pub const LEVEL_MULTIPLIER: f64 = 1.15;

/// A day qualifies for the streak at this completion percentage or higher.
pub const QUALIFY_PERCENT: i64 = 50;

/// (level, min lifetime XP, min current streak). Must be sorted by level.
pub const LEVELS: &[(i64, i64, i64)] = &[
    (1, 0, 0),
    (2, 100, 3),
    (3, 300, 7),
    (4, 700, 14),
    (5, 1_500, 30),
    (6, 3_000, 60),
    (7, 6_000, 100),
];
