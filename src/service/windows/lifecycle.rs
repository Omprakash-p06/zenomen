//! Windows Service Control Manager (SCM) integration and lifecycle (WIN-03).

use crate::service::types::HardwareControlError;

pub const SERVICE_NAME: &str = "ZenOmenHardwareService";
pub const SERVICE_DISPLAY_NAME: &str = "ZenOmen Hardware Control Service";

/// Register zenomen-hw.exe as an SCM-managed Windows service (WIN-03).
pub fn register_service() -> Result<(), HardwareControlError> {
    #[cfg(windows)]
    {
        use windows_sys::Win32::System::Services::{
            CloseServiceHandle, CreateServiceW, OpenSCManagerW, SC_MANAGER_ALL_ACCESS,
            SERVICE_ALL_ACCESS, SERVICE_AUTO_START, SERVICE_ERROR_NORMAL,
            SERVICE_WIN32_OWN_PROCESS,
        };

        let exe_path = std::env::current_exe()
            .map_err(|e| HardwareControlError::IoError(format!("Failed to get exe path: {e}")))?;
        let exe_wide: Vec<u16> = exe_path
            .to_string_lossy()
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        let name_wide: Vec<u16> = SERVICE_NAME
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        let display_wide: Vec<u16> = SERVICE_DISPLAY_NAME
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();

        unsafe {
            let scm = OpenSCManagerW(std::ptr::null(), std::ptr::null(), SC_MANAGER_ALL_ACCESS);
            if scm.is_null() {
                return Err(HardwareControlError::AccessDenied(
                    "Administrator privileges required to open Service Control Manager".to_string(),
                ));
            }

            let service = CreateServiceW(
                scm,
                name_wide.as_ptr(),
                display_wide.as_ptr(),
                SERVICE_ALL_ACCESS,
                SERVICE_WIN32_OWN_PROCESS,
                SERVICE_AUTO_START,
                SERVICE_ERROR_NORMAL,
                exe_wide.as_ptr(),
                std::ptr::null(),
                std::ptr::null_mut(),
                std::ptr::null(),
                std::ptr::null(), // LocalSystem
                std::ptr::null(),
            );

            CloseServiceHandle(scm);

            if service.is_null() {
                return Err(HardwareControlError::IoError(
                    "Failed to create service in SCM".to_string(),
                ));
            }

            CloseServiceHandle(service);
            Ok(())
        }
    }
    #[cfg(not(windows))]
    {
        Err(HardwareControlError::InterfaceMissing(
            "SCM services are only supported on Windows".to_string(),
        ))
    }
}

/// Uninstall zenomen-hw from the Service Control Manager.
pub fn uninstall_service() -> Result<(), HardwareControlError> {
    #[cfg(windows)]
    {
        use windows_sys::Win32::System::Services::{
            CloseServiceHandle, DeleteService, OpenSCManagerW, OpenServiceW, SC_MANAGER_ALL_ACCESS,
            SERVICE_ALL_ACCESS,
        };

        let name_wide: Vec<u16> = SERVICE_NAME
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();

        unsafe {
            let scm = OpenSCManagerW(std::ptr::null(), std::ptr::null(), SC_MANAGER_ALL_ACCESS);
            if scm.is_null() {
                return Err(HardwareControlError::AccessDenied(
                    "Administrator privileges required to open Service Control Manager".to_string(),
                ));
            }

            let service = OpenServiceW(scm, name_wide.as_ptr(), SERVICE_ALL_ACCESS);
            if service.is_null() {
                CloseServiceHandle(scm);
                return Err(HardwareControlError::IoError(
                    "Service not found in SCM".to_string(),
                ));
            }

            let del_res = DeleteService(service);
            CloseServiceHandle(service);
            CloseServiceHandle(scm);

            if del_res == 0 {
                return Err(HardwareControlError::IoError(
                    "Failed to mark service for deletion".to_string(),
                ));
            }

            Ok(())
        }
    }
    #[cfg(not(windows))]
    {
        Err(HardwareControlError::InterfaceMissing(
            "SCM services are only supported on Windows".to_string(),
        ))
    }
}

#[cfg(windows)]
static SERVICE_STATUS_HANDLE_GLOBAL: std::sync::atomic::AtomicIsize =
    std::sync::atomic::AtomicIsize::new(0);

#[cfg(windows)]
static SERVICE_STOP_SIGNAL: std::sync::OnceLock<std::sync::Arc<std::sync::atomic::AtomicBool>> =
    std::sync::OnceLock::new();

#[cfg(windows)]
fn get_stop_signal() -> std::sync::Arc<std::sync::atomic::AtomicBool> {
    SERVICE_STOP_SIGNAL
        .get_or_init(|| std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)))
        .clone()
}

/// Report the current status of the service to the SCM.
#[cfg(windows)]
pub fn report_service_status(
    status_handle: windows_sys::Win32::System::Services::SERVICE_STATUS_HANDLE,
    current_state: u32,
    win32_exit_code: u32,
    wait_hint: u32,
) {
    use windows_sys::Win32::System::Services::{
        SERVICE_ACCEPT_SHUTDOWN, SERVICE_ACCEPT_STOP, SERVICE_RUNNING, SERVICE_START_PENDING,
        SERVICE_STATUS, SERVICE_STOPPED, SERVICE_WIN32_OWN_PROCESS, SetServiceStatus,
    };

    static CHECK_POINT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(1);

    let controls_accepted = if current_state == SERVICE_START_PENDING {
        0
    } else {
        SERVICE_ACCEPT_STOP | SERVICE_ACCEPT_SHUTDOWN
    };

    let check_point = if current_state == SERVICE_RUNNING || current_state == SERVICE_STOPPED {
        0
    } else {
        CHECK_POINT.fetch_add(1, std::sync::atomic::Ordering::SeqCst)
    };

    let status = SERVICE_STATUS {
        dwServiceType: SERVICE_WIN32_OWN_PROCESS,
        dwCurrentState: current_state,
        dwControlsAccepted: controls_accepted,
        dwWin32ExitCode: win32_exit_code,
        dwServiceSpecificExitCode: 0,
        dwCheckPoint: check_point,
        dwWaitHint: wait_hint,
    };

    unsafe {
        SetServiceStatus(status_handle, &status);
    }
}

#[cfg(windows)]
unsafe extern "system" fn service_ctrl_handler(
    dw_control: u32,
    _dw_event_type: u32,
    _lp_event_data: *mut std::ffi::c_void,
    _lp_context: *mut std::ffi::c_void,
) -> u32 {
    use windows_sys::Win32::Foundation::{ERROR_CALL_NOT_IMPLEMENTED, NO_ERROR};
    use windows_sys::Win32::System::Services::{
        SERVICE_CONTROL_INTERROGATE, SERVICE_CONTROL_SHUTDOWN, SERVICE_CONTROL_STOP,
        SERVICE_STOP_PENDING,
    };

    let handle = SERVICE_STATUS_HANDLE_GLOBAL.load(std::sync::atomic::Ordering::SeqCst);

    match dw_control {
        SERVICE_CONTROL_STOP | SERVICE_CONTROL_SHUTDOWN => {
            tracing::info!("Received SCM stop/shutdown signal");
            let stop = get_stop_signal();
            stop.store(true, std::sync::atomic::Ordering::SeqCst);
            if handle != 0 {
                report_service_status(handle as _, SERVICE_STOP_PENDING, NO_ERROR, 3000);
            }
            NO_ERROR
        }
        SERVICE_CONTROL_INTERROGATE => NO_ERROR,
        _ => ERROR_CALL_NOT_IMPLEMENTED,
    }
}

#[cfg(windows)]
unsafe extern "system" fn service_main(
    _dw_num_service_args: u32,
    _lp_service_arg_vectors: *mut windows_sys::core::PWSTR,
) {
    use windows_sys::Win32::Foundation::NO_ERROR;
    use windows_sys::Win32::System::Services::{
        RegisterServiceCtrlHandlerExW, SERVICE_RUNNING, SERVICE_START_PENDING, SERVICE_STOPPED,
    };

    let name_wide: Vec<u16> = SERVICE_NAME
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();

    let status_handle = unsafe {
        RegisterServiceCtrlHandlerExW(
            name_wide.as_ptr(),
            Some(service_ctrl_handler),
            std::ptr::null_mut(),
        )
    };

    if status_handle.is_null() {
        tracing::error!("RegisterServiceCtrlHandlerExW failed");
        return;
    }

    SERVICE_STATUS_HANDLE_GLOBAL.store(status_handle as isize, std::sync::atomic::Ordering::SeqCst);

    report_service_status(status_handle, SERVICE_START_PENDING, NO_ERROR, 3000);

    let stop_signal = get_stop_signal();
    report_service_status(status_handle, SERVICE_RUNNING, NO_ERROR, 0);

    tracing::info!("Windows hardware service running under SCM dispatcher");

    if let Err(e) = crate::service::windows::server::run_pipe_server(stop_signal) {
        tracing::error!("Named pipe server exited with error: {e}");
    }

    report_service_status(status_handle, SERVICE_STOPPED, NO_ERROR, 0);
    tracing::info!("Windows hardware service stopped");
}

/// Run the hardware control server directly in console mode (non-SCM).
pub fn run_console_service() -> Result<(), HardwareControlError> {
    #[cfg(windows)]
    {
        tracing::info!("Starting Windows hardware service in console mode");
        let stop_signal = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        crate::service::windows::server::run_pipe_server(stop_signal)
    }
    #[cfg(not(windows))]
    {
        Err(HardwareControlError::InterfaceMissing(
            "Windows service lifecycle is only supported on Windows".to_string(),
        ))
    }
}

/// Start service execution under SCM dispatcher, with automatic fallback to console mode.
pub fn run_service() -> Result<(), HardwareControlError> {
    #[cfg(windows)]
    {
        use windows_sys::Win32::Foundation::ERROR_FAILED_SERVICE_CONTROLLER_CONNECT;
        use windows_sys::Win32::System::Services::{
            SERVICE_TABLE_ENTRYW, StartServiceCtrlDispatcherW,
        };

        let mut name_wide: Vec<u16> = SERVICE_NAME
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();

        let service_table = [
            SERVICE_TABLE_ENTRYW {
                lpServiceName: name_wide.as_mut_ptr(),
                lpServiceProc: Some(service_main),
            },
            SERVICE_TABLE_ENTRYW {
                lpServiceName: std::ptr::null_mut(),
                lpServiceProc: None,
            },
        ];

        let success = unsafe { StartServiceCtrlDispatcherW(service_table.as_ptr()) };

        if success == 0 {
            let err = unsafe { windows_sys::Win32::Foundation::GetLastError() };
            if err == ERROR_FAILED_SERVICE_CONTROLLER_CONNECT {
                tracing::info!(
                    "Not launched by Service Control Manager; falling back to console mode"
                );
                return run_console_service();
            } else {
                return Err(HardwareControlError::IoError(format!(
                    "StartServiceCtrlDispatcherW failed with code {err}"
                )));
            }
        }

        Ok(())
    }
    #[cfg(not(windows))]
    {
        Err(HardwareControlError::InterfaceMissing(
            "Windows service lifecycle is only supported on Windows".to_string(),
        ))
    }
}
