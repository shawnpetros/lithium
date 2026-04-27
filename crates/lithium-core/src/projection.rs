//! Month-end projection logic.
//!
//! Given the variable spend so far this month + days elapsed + total days in month,
//! plus declared fixed monthly costs, compute a projected end-of-month total.

use chrono::{Datelike, NaiveDate, Utc};

/// Inputs for a month-end projection.
#[derive(Debug, Clone, Copy)]
pub struct MonthProjectionInput {
    /// Total variable cost so far this month, in USD.
    pub variable_to_date: f64,
    /// Total fixed cost for this month (e.g., Max plan), in USD.
    pub fixed_total: f64,
    /// Days elapsed in the current month, including today.
    pub days_elapsed: u32,
    /// Total days in the current month.
    pub days_in_month: u32,
}

/// Result of a month-end projection.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MonthProjectionOutput {
    /// Variable spend to date.
    pub variable_to_date: f64,
    /// Average variable spend per day so far.
    pub variable_daily_avg: f64,
    /// Fixed total (just passed through).
    pub fixed_total: f64,
    /// Projected end-of-month variable total = avg * days_in_month.
    pub projected_variable_eom: f64,
    /// Projected end-of-month total = projected_variable_eom + fixed_total.
    pub projected_total_eom: f64,
}

/// Compute a month-end projection from the given inputs.
///
/// Edge cases:
/// - `days_elapsed == 0` returns zeroes for daily avg and projected variable
///   (we cannot project from zero history).
pub fn project_month(input: MonthProjectionInput) -> MonthProjectionOutput {
    let daily_avg = if input.days_elapsed == 0 {
        0.0
    } else {
        input.variable_to_date / input.days_elapsed as f64
    };
    let projected_variable = daily_avg * input.days_in_month as f64;
    let projected_total = projected_variable + input.fixed_total;
    MonthProjectionOutput {
        variable_to_date: input.variable_to_date,
        variable_daily_avg: daily_avg,
        fixed_total: input.fixed_total,
        projected_variable_eom: projected_variable,
        projected_total_eom: projected_total,
    }
}

/// Number of days in a given (year, month) tuple.
pub fn days_in_month(year: i32, month: u32) -> u32 {
    let next_first = if month == 12 {
        NaiveDate::from_ymd_opt(year + 1, 1, 1)
    } else {
        NaiveDate::from_ymd_opt(year, month + 1, 1)
    };
    let this_first = NaiveDate::from_ymd_opt(year, month, 1);
    match (this_first, next_first) {
        (Some(a), Some(b)) => (b - a).num_days() as u32,
        _ => 30, // pathological fallback; should never hit
    }
}

/// Days elapsed in the given (year, month) up to and including the given UTC `today`.
/// If `today` is in a different month, returns the full days-in-month.
pub fn days_elapsed(year: i32, month: u32, today: NaiveDate) -> u32 {
    if today.year() != year || today.month() != month {
        return days_in_month(year, month);
    }
    today.day()
}

/// Convenience: pull (year, month) from the current UTC date.
pub fn current_year_month() -> (i32, u32) {
    let now = Utc::now();
    (now.year(), now.month())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn days_in_month_known() {
        assert_eq!(days_in_month(2026, 1), 31);
        assert_eq!(days_in_month(2026, 2), 28); // not a leap year
        assert_eq!(days_in_month(2024, 2), 29); // leap year
        assert_eq!(days_in_month(2026, 4), 30);
    }

    #[test]
    fn projection_zero_days() {
        let out = project_month(MonthProjectionInput {
            variable_to_date: 0.0,
            fixed_total: 200.0,
            days_elapsed: 0,
            days_in_month: 30,
        });
        assert_eq!(out.variable_daily_avg, 0.0);
        assert_eq!(out.projected_variable_eom, 0.0);
        assert_eq!(out.projected_total_eom, 200.0);
    }

    #[test]
    fn projection_basic() {
        // $100 over 10 days, projected over 30 days, $200 fixed.
        let out = project_month(MonthProjectionInput {
            variable_to_date: 100.0,
            fixed_total: 200.0,
            days_elapsed: 10,
            days_in_month: 30,
        });
        assert!((out.variable_daily_avg - 10.0).abs() < 1e-9);
        assert!((out.projected_variable_eom - 300.0).abs() < 1e-9);
        assert!((out.projected_total_eom - 500.0).abs() < 1e-9);
    }

    #[test]
    fn projection_partial_month() {
        // Day 15 of 30 with $90 variable, no fixed.
        let out = project_month(MonthProjectionInput {
            variable_to_date: 90.0,
            fixed_total: 0.0,
            days_elapsed: 15,
            days_in_month: 30,
        });
        assert!((out.variable_daily_avg - 6.0).abs() < 1e-9);
        assert!((out.projected_variable_eom - 180.0).abs() < 1e-9);
    }

    #[test]
    fn days_elapsed_within_month() {
        let today = NaiveDate::from_ymd_opt(2026, 4, 27).unwrap();
        assert_eq!(days_elapsed(2026, 4, today), 27);
    }

    #[test]
    fn days_elapsed_after_month() {
        let today = NaiveDate::from_ymd_opt(2026, 5, 1).unwrap();
        assert_eq!(days_elapsed(2026, 4, today), 30);
    }
}
