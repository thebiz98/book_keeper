//! The single "today" seam for services (FR-28, DD-58). "Today" is the shop time zone from app
//! config (`timeZone`, default Asia/Kolkata), independent of the PC's zone setting; tests pin a day
//! with `with_today` / `TodayGuard`. The override is per thread, so parallel tests never interfere.

use std::cell::RefCell;
use std::sync::OnceLock;

use chrono::{DateTime, Utc};
use chrono_tz::Tz;

static SHOP_TZ: OnceLock<Tz> = OnceLock::new();

/// Sets the shop time zone once at startup (first call wins; later calls are ignored).
pub fn init_time_zone(tz: Tz) {
    let _ = SHOP_TZ.set(tz);
}

/// The shop time zone (Asia/Kolkata if never initialised, so tests need no setup).
pub fn shop_tz() -> Tz {
    SHOP_TZ.get().copied().unwrap_or(chrono_tz::Asia::Kolkata)
}

/// Current instant in the shop time zone.
pub fn now_local() -> DateTime<Tz> {
    Utc::now().with_timezone(&shop_tz())
}

thread_local! {
    static FIXED_TODAY: RefCell<Option<String>> = const { RefCell::new(None) };
}

/// Today's shop-zone date as `YYYY-MM-DD` (the pinned day when a test has set one on this thread).
pub fn today_local() -> String {
    FIXED_TODAY
        .with(|f| f.borrow().clone())
        .unwrap_or_else(|| now_local().format("%Y-%m-%d").to_string())
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
    fn default_zone_is_kolkata_plus_0530() {
        use chrono::Offset;
        assert_eq!(shop_tz(), chrono_tz::Asia::Kolkata);
        assert_eq!(now_local().offset().fix().local_minus_utc(), 19_800);
    }

    #[test]
    fn today_is_shop_date_when_not_pinned() {
        // Bracket the call so an IST midnight rollover cannot flake the test.
        let a = now_local().format("%Y-%m-%d").to_string();
        let t = today_local();
        let b = now_local().format("%Y-%m-%d").to_string();
        assert!(t == a || t == b);
    }

    #[test]
    fn pin_and_restore() {
        let real = today_local();
        with_today("2030-01-02", || assert_eq!(today_local(), "2030-01-02"));
        assert_eq!(today_local(), real);
    }
}
