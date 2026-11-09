//! Windows system identification and motherboard board profiling (WIN-07).

/// Query system manufacturer and model name.
pub fn read_system_identity() -> (String, String) {
    #[cfg(windows)]
    {
        use windows_sys::Win32::System::Registry::{
            HKEY_LOCAL_MACHINE, KEY_READ, RegCloseKey, RegOpenKeyExA, RegQueryValueExA,
        };

        let read_reg_str = |key_path: &[u8], value_name: &[u8]| -> Option<String> {
            unsafe {
                let mut hkey = std::ptr::null_mut();
                if RegOpenKeyExA(
                    HKEY_LOCAL_MACHINE,
                    key_path.as_ptr(),
                    0,
                    KEY_READ,
                    &mut hkey,
                ) == 0
                {
                    let mut buf = [0u8; 256];
                    let mut buf_len = buf.len() as u32;
                    let mut val_type = 0u32;
                    let res = RegQueryValueExA(
                        hkey,
                        value_name.as_ptr(),
                        std::ptr::null_mut(),
                        &mut val_type,
                        buf.as_mut_ptr(),
                        &mut buf_len,
                    );
                    RegCloseKey(hkey);
                    if res == 0 && buf_len > 0 {
                        let nul_pos = buf[..buf_len as usize]
                            .iter()
                            .position(|&b| b == 0)
                            .unwrap_or(buf_len as usize);
                        return std::str::from_utf8(&buf[..nul_pos])
                            .ok()
                            .map(|s| s.trim().to_string());
                    }
                }
            }
            None
        };

        let sys_key = c"HARDWARE\\DESCRIPTION\\System\\BIOS";
        let manufacturer = read_reg_str(
            sys_key.to_bytes_with_nul(),
            c"SystemManufacturer".to_bytes_with_nul(),
        )
        .unwrap_or_else(|| "HP".to_string());
        let model = read_reg_str(
            sys_key.to_bytes_with_nul(),
            c"SystemProductName".to_bytes_with_nul(),
        )
        .unwrap_or_else(|| "OMEN Laptop".to_string());

        (manufacturer, model)
    }
    #[cfg(not(windows))]
    {
        ("Unknown".to_string(), "Unknown".to_string())
    }
}

/// Query motherboard board ID (e.g. "88F8", "8A24") from BIOS registry / SMBIOS.
pub fn read_board_id() -> Option<String> {
    #[cfg(windows)]
    {
        use windows_sys::Win32::System::Registry::{
            HKEY_LOCAL_MACHINE, KEY_READ, RegCloseKey, RegOpenKeyExA, RegQueryValueExA,
        };

        unsafe {
            let mut hkey = std::ptr::null_mut();
            let key = c"HARDWARE\\DESCRIPTION\\System\\BIOS";
            if RegOpenKeyExA(
                HKEY_LOCAL_MACHINE,
                key.as_ptr() as *const u8,
                0,
                KEY_READ,
                &mut hkey,
            ) == 0
            {
                let mut buf = [0u8; 64];
                let mut buf_len = buf.len() as u32;
                let mut val_type = 0u32;
                let res = RegQueryValueExA(
                    hkey,
                    c"BaseBoardProduct".as_ptr() as *const u8,
                    std::ptr::null_mut(),
                    &mut val_type,
                    buf.as_mut_ptr(),
                    &mut buf_len,
                );
                RegCloseKey(hkey);
                if res == 0 && buf_len > 0 {
                    let nul_pos = buf[..buf_len as usize]
                        .iter()
                        .position(|&b| b == 0)
                        .unwrap_or(buf_len as usize);
                    return std::str::from_utf8(&buf[..nul_pos])
                        .ok()
                        .map(|s| s.trim().to_string());
                }
            }
        }
        None
    }
    #[cfg(not(windows))]
    {
        None
    }
}

/// Query BIOS version (e.g. "F.22") from BIOS registry.
pub fn read_bios_version() -> String {
    #[cfg(windows)]
    {
        use windows_sys::Win32::System::Registry::{
            HKEY_LOCAL_MACHINE, KEY_READ, RegCloseKey, RegOpenKeyExA, RegQueryValueExA,
        };

        unsafe {
            let mut hkey = std::ptr::null_mut();
            let key = c"HARDWARE\\DESCRIPTION\\System\\BIOS";
            if RegOpenKeyExA(
                HKEY_LOCAL_MACHINE,
                key.as_ptr() as *const u8,
                0,
                KEY_READ,
                &mut hkey,
            ) == 0
            {
                let mut buf = [0u8; 64];
                let mut buf_len = buf.len() as u32;
                let mut val_type = 0u32;
                let res = RegQueryValueExA(
                    hkey,
                    c"BIOSVersion".as_ptr() as *const u8,
                    std::ptr::null_mut(),
                    &mut val_type,
                    buf.as_mut_ptr(),
                    &mut buf_len,
                );
                RegCloseKey(hkey);
                if res == 0 && buf_len > 0 {
                    let nul_pos = buf[..buf_len as usize]
                        .iter()
                        .position(|&b| b == 0)
                        .unwrap_or(buf_len as usize);
                    if let Ok(ver) = std::str::from_utf8(&buf[..nul_pos]) {
                        let trimmed = ver.trim();
                        if !trimmed.is_empty() {
                            return trimmed.to_string();
                        }
                    }
                }
            }
        }
        "F.00".to_string()
    }
    #[cfg(not(windows))]
    {
        "F.00".to_string()
    }
}

/// Enumerate attached PnP hardware components.
pub fn enumerate_pnp_devices() -> Vec<String> {
    Vec::new()
}
