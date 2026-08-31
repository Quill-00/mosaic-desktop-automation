use crate::model::*;
use crate::runner;
use crate::state::{lk, Shared};
use chrono::{DateTime, Datelike, Local};
use std::time::Duration;
use tauri::AppHandle;

/// Ticks once per second and launches any due time-based task. Event-based
/// (watch) and manual triggers are handled elsewhere; this only covers Interval
/// and Daily. The platform is the only thing that fires tasks.
pub fn run_loop(state: Shared, app: AppHandle) {
    loop {
        std::thread::sleep(Duration::from_secs(1));
        let tasks: Vec<Task> = { lk(&state.db).tasks.clone() };
        let now = Local::now();
        for task in tasks {
            if !task.active || !task.enabled {
                continue;
            }
            let last = lk(&state.last_runs).get(&task.id).cloned();
            let due = match &task.trigger {
                Trigger::Interval { every_secs } => interval_due(last, now, (*every_secs).max(1)),
                Trigger::Daily { at } => match parse_hhmm(at) {
                    Some((h, m)) => daily_due(last, now, h, m),
                    None => false,
                },
                Trigger::Weekly { days, at } => match parse_hhmm(at) {
                    Some((h, m)) => weekly_due(last, now, days, h, m),
                    None => false,
                },
                Trigger::Monthly { day, at } => match parse_hhmm(at) {
                    Some((h, m)) => monthly_due(last, now, *day, h, m),
                    None => false,
                },
                _ => false,
            };
            if due {
                lk(&state.last_runs).insert(task.id.clone(), now);
                runner::run_task(state.clone(), app.clone(), task.clone(), "定时".into());
            }
        }
    }
}

fn interval_due(last: Option<DateTime<Local>>, now: DateTime<Local>, every: u64) -> bool {
    match last {
        Some(t) => (now - t).num_seconds() >= every as i64,
        None => true,
    }
}

/// Due when we are past a target time today and have not already run since it.
/// Robust to missing the exact minute (sleep / clock jump) — unlike a strict
/// `hour == h && minute == m` check.
fn target_due(last: Option<DateTime<Local>>, now: DateTime<Local>, h: u32, m: u32) -> bool {
    let target = match now.date_naive().and_hms_opt(h, m, 0) {
        Some(t) => t,
        None => return false,
    };
    if now.naive_local() < target {
        return false;
    }
    match last {
        Some(t) => t.naive_local() < target,
        None => true,
    }
}

fn daily_due(last: Option<DateTime<Local>>, now: DateTime<Local>, h: u32, m: u32) -> bool {
    target_due(last, now, h, m)
}

fn weekly_due(
    last: Option<DateTime<Local>>,
    now: DateTime<Local>,
    days: &[u8],
    h: u32,
    m: u32,
) -> bool {
    let wd = now.weekday().num_days_from_sunday() as u8;
    if !days.contains(&wd) {
        return false;
    }
    target_due(last, now, h, m)
}

fn monthly_due(
    last: Option<DateTime<Local>>,
    now: DateTime<Local>,
    day: u8,
    h: u32,
    m: u32,
) -> bool {
    let dim = days_in_month(now.year(), now.month());
    let eff = (day as u32).clamp(1, dim);
    if now.day() != eff {
        return false;
    }
    target_due(last, now, h, m)
}

fn days_in_month(year: i32, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if (year % 4 == 0 && year % 100 != 0) || year % 400 == 0 {
                29
            } else {
                28
            }
        }
        _ => 30,
    }
}

fn parse_hhmm(s: &str) -> Option<(u32, u32)> {
    let mut it = s.split(':');
    let h: u32 = it.next()?.trim().parse().ok()?;
    let m: u32 = it.next()?.trim().parse().ok()?;
    if h < 24 && m < 60 {
        Some((h, m))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn at(h: u32, m: u32, s: u32) -> DateTime<Local> {
        Local.with_ymd_and_hms(2026, 6, 21, h, m, s).unwrap()
    }

    #[test]
    fn hhmm_parsing() {
        assert_eq!(parse_hhmm("09:00"), Some((9, 0)));
        assert_eq!(parse_hhmm("23:59"), Some((23, 59)));
        assert_eq!(parse_hhmm("24:00"), None);
        assert_eq!(parse_hhmm("bad"), None);
    }

    #[test]
    fn interval_fires_first_time_then_waits() {
        let now = at(10, 0, 0);
        assert!(interval_due(None, now, 60));
        assert!(!interval_due(Some(at(9, 59, 30)), now, 60));
        assert!(interval_due(Some(at(9, 59, 0)), now, 60));
    }

    #[test]
    fn daily_fires_after_target_once_per_day() {
        // before target
        assert!(!daily_due(None, at(8, 0, 0), 9, 0));
        // after target, never ran
        assert!(daily_due(None, at(9, 30, 0), 9, 0));
        // after target, already ran today
        assert!(!daily_due(Some(at(9, 0, 5)), at(9, 30, 0), 9, 0));
        // after target, last ran was yesterday
        let yesterday = Local.with_ymd_and_hms(2026, 6, 20, 9, 0, 0).unwrap();
        assert!(daily_due(Some(yesterday), at(9, 30, 0), 9, 0));
    }

    #[test]
    fn weekly_runs_on_matching_weekday() {
        let now = at(10, 0, 0);
        let wd = now.weekday().num_days_from_sunday() as u8;
        assert!(weekly_due(None, now, &[wd], 9, 0));
        assert!(!weekly_due(None, now, &[(wd + 1) % 7], 9, 0));
        assert!(!weekly_due(None, at(8, 0, 0), &[wd], 9, 0));
    }

    #[test]
    fn monthly_runs_on_day_and_clamps_to_month_end() {
        // 2026-06-21, June has 30 days.
        assert!(monthly_due(None, at(10, 0, 0), 21, 9, 0));
        assert!(!monthly_due(None, at(10, 0, 0), 20, 9, 0));
        let jun30 = Local.with_ymd_and_hms(2026, 6, 30, 10, 0, 0).unwrap();
        assert!(monthly_due(None, jun30, 31, 9, 0)); // 31 clamps to 30 == today
    }
}
