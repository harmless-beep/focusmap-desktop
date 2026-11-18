//! Small platform queries used by the UI: memory, CPU time, motion preference.

/// Current working set of this process in bytes.
pub fn working_set_bytes() -> u64 {
    #[cfg(windows)]
    {
        use windows_sys::Win32::System::ProcessStatus::{
            K32GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS,
        };
        use windows_sys::Win32::System::Threading::GetCurrentProcess;
        let mut counters: PROCESS_MEMORY_COUNTERS = unsafe { std::mem::zeroed() };
        counters.cb = std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32;
        let ok = unsafe {
            K32GetProcessMemoryInfo(GetCurrentProcess(), &mut counters, counters.cb)
        };
        if ok != 0 {
            return counters.WorkingSetSize as u64;
        }
        0
    }
    #[cfg(not(windows))]
    {
        0
    }
}

/// Total CPU time used by this process, in 100 ns units. Sample it twice and
/// divide by the elapsed wall time to get a CPU percentage.
pub fn cpu_time_100ns() -> u64 {
    #[cfg(windows)]
    {
        use windows_sys::Win32::Foundation::FILETIME;
        use windows_sys::Win32::System::Threading::{GetCurrentProcess, GetProcessTimes};
        let mut creation = FILETIME {
            dwLowDateTime: 0,
            dwHighDateTime: 0,
        };
        let mut exit = creation;
        let mut kernel = creation;
        let mut user = creation;
        let ok = unsafe {
            GetProcessTimes(
                GetCurrentProcess(),
                &mut creation,
                &mut exit,
                &mut kernel,
                &mut user,
            )
        };
        if ok == 0 {
            return 0;
        }
        let to_u64 = |ft: FILETIME| ((ft.dwHighDateTime as u64) << 32) | ft.dwLowDateTime as u64;
        to_u64(kernel) + to_u64(user)
    }
    #[cfg(not(windows))]
    {
        0
    }
}

/// True when the user asked the OS to reduce animations.
pub fn system_reduced_motion() -> bool {
    #[cfg(windows)]
    {
        use windows_sys::Win32::UI::WindowsAndMessaging::{
            SystemParametersInfoW, SPI_GETCLIENTAREAANIMATION,
        };
        let mut enabled = 1i32;
        let ok = unsafe {
            SystemParametersInfoW(
                SPI_GETCLIENTAREAANIMATION,
                0,
                &mut enabled as *mut i32 as *mut std::ffi::c_void,
                0,
            )
        };
        ok != 0 && enabled == 0
    }
    #[cfg(not(windows))]
    {
        false
    }
}

/// True when the system-wide app theme is dark.
pub fn system_prefers_dark() -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        use windows_sys::Win32::System::Registry::{
            RegCloseKey, RegOpenKeyExW, RegQueryValueExW, HKEY_CURRENT_USER, KEY_READ, REG_DWORD,
        };
        let subkey: Vec<u16> = std::ffi::OsStr::new(
            "Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize",
        )
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
        let name: Vec<u16> = std::ffi::OsStr::new("AppsUseLightTheme")
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
        let mut key = std::ptr::null_mut();
        let ok = unsafe { RegOpenKeyExW(HKEY_CURRENT_USER, subkey.as_ptr(), 0, KEY_READ, &mut key) };
        if ok != 0 {
            return true;
        }
        let mut value = 1u32;
        let mut size = std::mem::size_of::<u32>() as u32;
        let mut kind = 0u32;
        let ok = unsafe {
            RegQueryValueExW(
                key,
                name.as_ptr(),
                std::ptr::null(),
                &mut kind,
                &mut value as *mut u32 as *mut u8,
                &mut size,
            )
        };
        unsafe { RegCloseKey(key) };
        ok == 0 && kind == REG_DWORD && value == 0
    }
    #[cfg(not(windows))]
    {
        false
    }
}
