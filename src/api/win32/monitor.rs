use std::collections::VecDeque;
use std::mem;

use native_monitor::NativeMonitorId;
use winapi::um::winnt::WCHAR;
use winapi::um::wingdi::{DEVMODEW, DISPLAY_DEVICE_PRIMARY_DEVICE};
use winapi::um::wingdi::{DISPLAY_DEVICE_MIRRORING_DRIVER, DISPLAY_DEVICE_ACTIVE};
use winapi::um::wingdi::{DISPLAY_DEVICEW};
use winapi::shared::windef::POINTL;
use winapi::shared::minwindef::{DWORD, WORD};
use winapi::um::winuser::{ENUM_CURRENT_SETTINGS, EnumDisplayDevicesW, EnumDisplaySettingsExW};

/// Win32 implementation of the main `MonitorId` object.
#[derive(Clone)]
pub struct MonitorId {
    /// The system name of the adapter.
    adapter_name: [WCHAR; 32],

    /// The system name of the monitor.
    monitor_name: String,

    /// Name to give to the user.
    readable_name: String,

    /// See the `StateFlags` element here:
    /// http://msdn.microsoft.com/en-us/library/dd183569(v=vs.85).aspx
    flags: DWORD,

    /// True if this is the primary monitor.
    primary: bool,

    /// The position of the monitor in pixels on the desktop.
    ///
    /// A window that is positionned at these coordinates will overlap the monitor.
    position: (u32, u32),

    /// The current resolution in pixels on the monitor.
    dimensions: (u32, u32),
}

struct DeviceEnumerator {
    parent_device: *const WCHAR,
    current_index: u32,
}

impl DeviceEnumerator {
    fn adapters() -> DeviceEnumerator {
        use std::ptr;
        DeviceEnumerator {
            parent_device: ptr::null(),
            current_index: 0
        }
    }

    fn monitors(adapter_name: *const WCHAR) -> DeviceEnumerator {
        DeviceEnumerator {
            parent_device: adapter_name,
            current_index: 0
        }
    }
}

impl Iterator for DeviceEnumerator {
    type Item = DISPLAY_DEVICEW;
    fn next(&mut self) -> Option<DISPLAY_DEVICEW> {
        use std::mem;
        loop {
            let mut output: DISPLAY_DEVICEW = unsafe { mem::zeroed() };
            output.cb = mem::size_of::<DISPLAY_DEVICEW>() as DWORD;

            if unsafe { EnumDisplayDevicesW(self.parent_device,
                self.current_index as DWORD, &mut output, 0) } == 0
            {
                // the device doesn't exist, which means we have finished enumerating
                break;
            }
            self.current_index += 1;

            if  (output.StateFlags & DISPLAY_DEVICE_ACTIVE) == 0 ||
                (output.StateFlags & DISPLAY_DEVICE_MIRRORING_DRIVER) != 0
            {
                // the device is not active
                // the Win32 api usually returns a lot of inactive devices
                continue;
            }

            return Some(output);
        }
        None
    }
}

fn wchar_as_string(wchar: &[WCHAR]) -> String {
    String::from_utf16_lossy(wchar)
        .trim_right_matches(0 as char)
        .to_string()
}

/// Win32 implementation of the main `get_available_monitors` function.
pub fn get_available_monitors() -> VecDeque<MonitorId> {
    // return value
    let mut result = VecDeque::new();

    for adapter in DeviceEnumerator::adapters() {
        // getting the position
        let (position, dimensions) = unsafe {
            let mut dev: DEVMODEW = mem::zeroed();
            dev.dmSize = mem::size_of::<DEVMODEW>() as WORD;

            if EnumDisplaySettingsExW(adapter.DeviceName.as_ptr(), 
                ENUM_CURRENT_SETTINGS,
                &mut dev, 0) == 0
            {
                continue;
            }

            let point: &POINTL = mem::transmute(&dev.u1);
            let position = (point.x as u32, point.y as u32);

            let dimensions = (dev.dmPelsWidth as u32, dev.dmPelsHeight as u32);

            (position, dimensions)
        };

        for (num, monitor) in DeviceEnumerator::monitors(adapter.DeviceName.as_ptr()).enumerate() {
            // adding to the resulting list
            result.push_back(MonitorId {
                adapter_name: adapter.DeviceName,
                monitor_name: wchar_as_string(&monitor.DeviceName),
                readable_name: wchar_as_string(&monitor.DeviceString),
                flags: monitor.StateFlags,
                primary: (adapter.StateFlags & DISPLAY_DEVICE_PRIMARY_DEVICE) != 0 &&
                         num == 0,
                position: position,
                dimensions: dimensions,
            });
        }
    }
    result
}

/// Win32 implementation of the main `get_primary_monitor` function.
pub fn get_primary_monitor() -> MonitorId {
    // we simply get all available monitors and return the one with the `PRIMARY_DEVICE` flag
    // TODO: it is possible to query the win32 API for the primary monitor, this should be done
    //  instead
    for monitor in get_available_monitors().into_iter() {
        if monitor.primary {
            return monitor;
        }
    }

    panic!("Failed to find the primary monitor")
}

impl MonitorId {
    /// See the docs if the crate root file.
    #[inline]
    pub fn get_name(&self) -> Option<String> {
        Some(self.readable_name.clone())
    }

    /// See the docs of the crate root file.
    #[inline]
    pub fn get_native_identifier(&self) -> NativeMonitorId {
        NativeMonitorId::Name(self.monitor_name.clone())
    }

    /// See the docs if the crate root file.
    #[inline]
    pub fn get_dimensions(&self) -> (u32, u32) {
        // TODO: retreive the dimensions every time this is called
        self.dimensions
    }

    /// This is a Win32-only function for `MonitorId` that returns the system name of the adapter
    /// device.
    #[inline]
    pub fn get_adapter_name(&self) -> &[WCHAR] {
        &self.adapter_name
    }

    /// This is a Win32-only function for `MonitorId` that returns the position of the
    ///  monitor on the desktop.
    /// A window that is positionned at these coordinates will overlap the monitor.
    #[inline]
    pub fn get_position(&self) -> (u32, u32) {
        self.position
    }
}
