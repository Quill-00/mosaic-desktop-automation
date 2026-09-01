const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
const VALUE_NAME: &str = "Mosaic";

#[cfg(windows)]
pub fn is_enabled() -> bool {
    read_entry(RUN_KEY, VALUE_NAME)
        .map(|value| !value.trim().is_empty())
        .unwrap_or(false)
}

#[cfg(windows)]
fn read_entry(key_path: &str, value_name: &str) -> std::io::Result<String> {
    use winreg::enums::HKEY_CURRENT_USER;
    use winreg::RegKey;

    RegKey::predef(HKEY_CURRENT_USER)
        .open_subkey(key_path)?
        .get_value(value_name)
}

#[cfg(windows)]
fn write_entry(key_path: &str, value_name: &str, value: Option<&str>) -> std::io::Result<()> {
    use winreg::enums::HKEY_CURRENT_USER;
    use winreg::RegKey;

    let (key, _) = RegKey::predef(HKEY_CURRENT_USER).create_subkey(key_path)?;
    match value {
        Some(value) => key.set_value(value_name, &value),
        None => match key.delete_value(value_name) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error),
        },
    }
}

#[cfg(windows)]
pub fn set_enabled(enabled: bool) -> Result<(), String> {
    if enabled {
        let executable = std::env::current_exe().map_err(|error| {
            format!(
                "{}: {error}",
                crate::locale::text("Unable to locate Mosaic", "无法定位 Mosaic")
            )
        })?;
        let command = format!("\"{}\"", executable.display());
        write_entry(RUN_KEY, VALUE_NAME, Some(&command)).map_err(|error| {
            format!(
                "{}: {error}",
                crate::locale::text("Unable to enable startup", "无法启用开机自动启动")
            )
        })
    } else {
        write_entry(RUN_KEY, VALUE_NAME, None).map_err(|error| {
            format!(
                "{}: {error}",
                crate::locale::text("Unable to disable startup", "无法关闭开机自动启动")
            )
        })
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::{read_entry, write_entry};
    use winreg::enums::HKEY_CURRENT_USER;
    use winreg::RegKey;

    #[test]
    fn startup_entry_can_be_enabled_and_removed() {
        let key_path = format!(r"Software\Mosaic\Tests\Autostart-{}", std::process::id());
        let command = r#""C:\Program Files\Mosaic\Mosaic.exe""#;

        if let Err(error) = write_entry(&key_path, "MosaicTest", Some(command)) {
            // Some CI/sandbox profiles intentionally deny HKCU writes. The
            // normal Windows integration run exercises the full round trip.
            assert_eq!(error.kind(), std::io::ErrorKind::PermissionDenied);
            return;
        }
        assert_eq!(read_entry(&key_path, "MosaicTest").unwrap(), command);
        write_entry(&key_path, "MosaicTest", None).unwrap();
        assert!(read_entry(&key_path, "MosaicTest").is_err());

        let _ = RegKey::predef(HKEY_CURRENT_USER).delete_subkey_all(&key_path);
    }
}

#[cfg(not(windows))]
pub fn is_enabled() -> bool {
    false
}

#[cfg(not(windows))]
pub fn set_enabled(enabled: bool) -> Result<(), String> {
    if enabled {
        Err("Launch at login is available only on Windows.".into())
    } else {
        Ok(())
    }
}
