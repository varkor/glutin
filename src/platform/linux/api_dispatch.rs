/*pub use api::x11::{Window, WindowProxy, MonitorId, get_available_monitors, get_primary_monitor};
pub use api::x11::{WaitEventsIterator, PollEventsIterator};*/

use std::collections::VecDeque;
use std::sync::Arc;

use ContextError;
use CreationError;
use CursorState;
use Event;
use GlAttributes;
use GlContext;
use MouseCursor;
use PixelFormat;
use PixelFormatRequirements;
use WindowAttributes;
use libc;

use api::x11;
use api::x11::XConnection;
use api::x11::XError;
use api::x11::XNotSupported;

#[derive(Clone, Default)]
pub struct PlatformSpecificWindowBuilderAttributes;

enum Backend {
    X(Arc<XConnection>),
    Error(XNotSupported),
}

lazy_static!(
    static ref BACKEND: Backend = {
        match XConnection::new(Some(x_error_callback)) {
            Ok(x) => Backend::X(Arc::new(x)),
            Err(e) => Backend::Error(e),
        }
    };
);

pub enum Window {
    #[doc(hidden)]
    X(x11::Window),
}

#[derive(Clone)]
pub enum WindowProxy {
    #[doc(hidden)]
    X(x11::WindowProxy),
}

impl WindowProxy {
    #[inline]
    pub fn wakeup_event_loop(&self) {
        match self {
            &WindowProxy::X(ref wp) => wp.wakeup_event_loop(),
        }
    }
}

#[derive(Clone)]
pub enum MonitorId {
    #[doc(hidden)]
    X(x11::MonitorId),
    #[doc(hidden)]
    None,
}

#[inline]
pub fn get_available_monitors() -> VecDeque<MonitorId> {
    match *BACKEND {
        Backend::X(ref connec) => x11::get_available_monitors(connec)
                                    .into_iter()
                                    .map(MonitorId::X)
                                    .collect(),
        Backend::Error(_) => { let mut d = VecDeque::new(); d.push_back(MonitorId::None); d},
    }
}

#[inline]
pub fn get_primary_monitor() -> MonitorId {
    match *BACKEND {
        Backend::X(ref connec) => MonitorId::X(x11::get_primary_monitor(connec)),
        Backend::Error(_) => MonitorId::None,
    }
}

impl MonitorId {
    #[inline]
    pub fn get_name(&self) -> Option<String> {
        match self {
            &MonitorId::X(ref m) => m.get_name(),
            &MonitorId::None => None,
        }
    }

    #[inline]
    pub fn get_native_identifier(&self) -> ::native_monitor::NativeMonitorId {
        match self {
            &MonitorId::X(ref m) => m.get_native_identifier(),
            &MonitorId::None => unimplemented!()        // FIXME:
        }
    }

    #[inline]
    pub fn get_dimensions(&self) -> (u32, u32) {
        match self {
            &MonitorId::X(ref m) => m.get_dimensions(),
            &MonitorId::None => (800, 600),     // FIXME:
        }
    }
}


pub use api::x11::{PollEventsIterator, WaitEventsIterator};

impl Window {
    #[inline]
    pub fn new(window: &WindowAttributes, pf_reqs: &PixelFormatRequirements,
               opengl: &GlAttributes<&Window>, _: &PlatformSpecificWindowBuilderAttributes)
               -> Result<Window, CreationError>
    {
        match *BACKEND {
            Backend::X(ref connec) => {
                let opengl = opengl.clone().map_sharing(|w| match w {
                    &Window::X(ref w) => w,
                });

                x11::Window::new(connec, window, pf_reqs, &opengl).map(Window::X)
            },

            Backend::Error(ref error) => Err(CreationError::NoBackendAvailable(Box::new(error.clone())))
        }
    }

    #[inline]
    pub fn set_title(&self, title: &str) {
        match self {
            &Window::X(ref w) => w.set_title(title),
        }
    }

    #[inline]
    pub fn show(&self) {
        match self {
            &Window::X(ref w) => w.show(),
        }
    }

    #[inline]
    pub fn hide(&self) {
        match self {
            &Window::X(ref w) => w.hide(),
        }
    }

    #[inline]
    pub fn get_position(&self) -> Option<(i32, i32)> {
        match self {
            &Window::X(ref w) => w.get_position(),
        }
    }

    #[inline]
    pub fn set_position(&self, x: i32, y: i32) {
        match self {
            &Window::X(ref w) => w.set_position(x, y),
        }
    }

    #[inline]
    pub fn get_inner_size(&self) -> Option<(u32, u32)> {
        match self {
            &Window::X(ref w) => w.get_inner_size(),
        }
    }

    #[inline]
    pub fn get_outer_size(&self) -> Option<(u32, u32)> {
        match self {
            &Window::X(ref w) => w.get_outer_size(),
        }
    }

    #[inline]
    pub fn set_inner_size(&self, x: u32, y: u32) {
        match self {
            &Window::X(ref w) => w.set_inner_size(x, y),
        }
    }

    #[inline]
    pub fn create_window_proxy(&self) -> WindowProxy {
        match self {
            &Window::X(ref w) => WindowProxy::X(w.create_window_proxy()),
        }
    }

    #[inline]
    pub fn poll_events(&self) -> PollEventsIterator {
        match self {
            &Window::X(ref w) => w.poll_events(),
        }
    }

    #[inline]
    pub fn wait_events(&self) -> WaitEventsIterator {
        match self {
            &Window::X(ref w) => w.wait_events(),
        }
    }

    #[inline]
    pub fn set_window_resize_callback(&mut self, callback: Option<fn(u32, u32)>) {
        match self {
            &mut Window::X(ref mut w) => w.set_window_resize_callback(callback),
        }
    }

    #[inline]
    pub fn set_cursor(&self, cursor: MouseCursor) {
        match self {
            &Window::X(ref w) => w.set_cursor(cursor),
        }
    }

    #[inline]
    pub fn set_cursor_state(&self, state: CursorState) -> Result<(), String> {
        match self {
            &Window::X(ref w) => w.set_cursor_state(state),
        }
    }

    #[inline]
    pub fn hidpi_factor(&self) -> f32 {
       match self {
            &Window::X(ref w) => w.hidpi_factor(),
        }
    }

    #[inline]
    pub fn set_cursor_position(&self, x: i32, y: i32) -> Result<(), ()> {
        match self {
            &Window::X(ref w) => w.set_cursor_position(x, y),
        }
    }

    #[inline]
    pub fn platform_display(&self) -> *mut libc::c_void {
        match self {
            &Window::X(ref w) => w.platform_display(),
        }
    }

    #[inline]
    pub fn platform_window(&self) -> *mut libc::c_void {
        match self {
            &Window::X(ref w) => w.platform_window(),
        }
    }
}

impl GlContext for Window {
    #[inline]
    unsafe fn make_current(&self) -> Result<(), ContextError> {
        match self {
            &Window::X(ref w) => w.make_current(),
        }
    }

    #[inline]
    fn is_current(&self) -> bool {
        match self {
            &Window::X(ref w) => w.is_current(),
        }
    }

    #[inline]
    fn get_proc_address(&self, addr: &str) -> *const () {
        match self {
            &Window::X(ref w) => w.get_proc_address(addr),
        }
    }

    #[inline]
    fn swap_buffers(&self) -> Result<(), ContextError> {
        match self {
            &Window::X(ref w) => w.swap_buffers(),
        }
    }

    #[inline]
    fn get_api(&self) -> ::Api {
        match self {
            &Window::X(ref w) => w.get_api(),
        }
    }

    #[inline]
    fn get_pixel_format(&self) -> PixelFormat {
        match self {
            &Window::X(ref w) => w.get_pixel_format(),
        }
    }
}

unsafe extern "C" fn x_error_callback(dpy: *mut x11::ffi::Display, event: *mut x11::ffi::XErrorEvent)
                                      -> libc::c_int
{
    use std::ffi::CStr;

    if let Backend::X(ref x) = *BACKEND {
        let mut buff: Vec<u8> = Vec::with_capacity(1024);
        (x.xlib.XGetErrorText)(dpy, (*event).error_code as i32, buff.as_mut_ptr() as *mut libc::c_char, buff.capacity() as i32);
        let description = CStr::from_ptr(buff.as_mut_ptr() as *const libc::c_char).to_string_lossy();

        let error = XError {
            description: description.into_owned(),
            error_code: (*event).error_code,
            request_code: (*event).request_code,
            minor_code: (*event).minor_code,
        };

        *x.latest_error.lock().unwrap() = Some(error);
    }

    0
}
