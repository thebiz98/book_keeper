//! The single "today" seam for services (FR-28, DD-58). "Today" is India time (UTC+05:30, no DST),
//! independent of the PC's zone setting; tests pin a day with `with_today` / `TodayGuard`. The
//! override is per thread, so parallel tests never interfere.

use std::cell::RefCell;

use chrono::{DateTime, FixedOffset, Utc};

/// India Standard Time offset from UTC, in seconds (+05:30).
pub const IST_OFFSET_SECS: i32 = 19_800;

/// Current instant in India time.
pub fn now_ist() -> DateTime<FixedOffset> {
    Utc::now().with_timezone(&FixedOffset::east_opt(IST_OFFSET_SECS).expect("valid IST offset"))
}

thread_local! {
    static FIXED_TODAY: RefCell<Option<String>> = const { RefCell::new(None) };
}

/// Today's India-time date as `YYYY-MM-DD` (the pinned day when a test has set one on this thread).
pub fn today_local() -> String {
    FIXED_TODAY
        .with(|f| f.borrow().clone())
        .unwrap_or_else(|| now_ist().format("%Y-%m-%d").to_string())
}

/// Pins "today" on the current thread until dropped.
pub struct TodayGuard {
    prev: Option<String>,
}

impl TodayGuard {
    pub fn new(today: &str) -> Self {
        chrono::NaiveDate::parse_from_str(today, "%Y-%m-%d").expect("pinned today must be YYYY-MM-DD");
        let prev = FIXED_TODAY.with(|f| f.borrow_mut().replace(today.to_string()));
        Self { prev }
    }
}

impl Drop for TodayGuard {
    fn drop(&mut self) {
        FIXED_TODAY.with(|f| *f.borrow_mut() = self.prev.take());
    }
}

/// Runs `f` with "today" pinned to `today`.
pub fn with_today<T>(today: &str, f: impl FnOnce() -> T) -> T {
    let _g = TodayGuard::new(today);
    f()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn now_ist_is_plus_0530() {
        assert_eq!(now_ist().offset().local_minus_utc(), 19_800);
    }

    #[test]
    fn today_is_ist_date_when_not_pinned() {
        // Bracket the call so an IST midnight rollover cannot flake the test.
        let a = now_ist().format("%Y-%m-%d").to_string();
        let t = today_local();
        let b = now_ist().format("%Y-%m-%d").to_string();
        assert!(t == a || t == b);
    }

    #[test]
    fn pin_and_restore() {
        let real = today_local();
        with_today("2030-01-02", || assert_eq!(today_local(), "2030-01-02"));
        assert_eq!(today_local(), real);
    }
}
