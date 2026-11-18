//! Start on boot via the per-user Windows Run key. No service, no admin rights.

use anyhow::Result;

const RUN_KEY: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Run";
const VALUE: &str = "FocusMap";

pub fn set_enabled(enabled: bool) -> Result<()> {
    #[cfg(windows)]
    {
        use winreg::enums::{HKEY_CURRENT_USER, KEY_READ, KEY_WRITE};
        use winreg::RegKey;

        let current_user = RegKey::predef(HKEY_CURRENT_USER);
        let key = current_user.open_subkey_with_flags(RUN_KEY, KEY_READ | KEY_WRITE)?;
        if enabled {
            let exe = std::env::current_exe()?;
            key.set_value(VALUE, &format!("\"{}\" --minimized", exe.display()))?;
        } else {
            let _ = key.delete_value(VALUE);
        }
    }
    #[cfg(not(windows))]
    {
        let _ = enabled;
    }
    Ok(())
}

pub fn is_enabled() -> bool {
    #[cfg(windows)]
    {
        use winreg::enums::HKEY_CURRENT_USER;
        use winreg::RegKey;

        let current_user = RegKey::predef(HKEY_CURRENT_USER);
        return current_user
            .open_subkey(RUN_KEY)
            .and_then(|key| key.get_value::<String, _>(VALUE))
            .is_ok();
    }
    #[cfg(not(windows))]
    {
        false
    }
}
