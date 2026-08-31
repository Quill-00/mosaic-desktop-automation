use chrono::Local;
use std::sync::atomic::{AtomicBool, Ordering};

static CHINESE: AtomicBool = AtomicBool::new(false);

pub fn locale_for_utc_offset(offset_seconds: i32) -> &'static str {
    if offset_seconds == 8 * 60 * 60 {
        "zh-CN"
    } else {
        "en"
    }
}

pub fn init_from_timezone() {
    let locale = locale_for_utc_offset(Local::now().offset().local_minus_utc());
    CHINESE.store(locale == "zh-CN", Ordering::Relaxed);
}

pub fn set(locale: &str) -> Result<(), String> {
    let chinese = match locale {
        "en" => false,
        "zh-CN" => true,
        _ => return Err("Unsupported locale".into()),
    };
    CHINESE.store(chinese, Ordering::Relaxed);
    Ok(())
}

pub fn text(english: &str, chinese: &str) -> String {
    if is_chinese() {
        chinese.into()
    } else {
        english.into()
    }
}

pub fn is_chinese() -> bool {
    CHINESE.load(Ordering::Relaxed)
}

#[cfg(test)]
mod tests {
    use super::locale_for_utc_offset;

    #[test]
    fn only_utc_eight_defaults_to_chinese() {
        assert_eq!(locale_for_utc_offset(8 * 60 * 60), "zh-CN");
        assert_eq!(locale_for_utc_offset(0), "en");
        assert_eq!(locale_for_utc_offset(-5 * 60 * 60), "en");
        assert_eq!(locale_for_utc_offset(9 * 60 * 60), "en");
    }
}
