#![cfg(target_os = "windows")]

use std::mem;
use std::ptr;
use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use std::sync::{
    Arc,
    Mutex
};
use std::sync::mpsc::Receiver;
use libc;
use ContextError;
use {CreationError, Event, MouseCursor};
use CursorState;
use GlAttributes;
use GlContext;

use Api;
use PixelFormat;
use PixelFormatRequirements;
use WindowAttributes;

pub use self::monitor::{MonitorId, get_available_monitors, get_primary_monitor};

use winapi::shared::windef::RECT;
use winapi::ctypes::wchar_t;
use winapi::um::winuser::{IDC_ARROW, IDC_CROSS, IDC_HAND, IDC_HELP, WM_DESTROY};
use winapi::um::winuser::{IDC_IBEAM, IDC_NO, IDC_SIZENS, IDC_SIZEWE, IDC_WAIT};
use winapi::um::winuser::{SWP_NOMOVE, SWP_NOREPOSITION, SWP_NOSIZE, SWP_NOZORDER};
use winapi::um::winuser::{GWL_EXSTYLE, GWL_STYLE, WINDOWPLACEMENT, PostMessageA};
use winapi::um::winuser::{SW_SHOW, SW_HIDE, RegisterWindowMessageA, DestroyWindow};
use winapi::um::winuser::{PostMessageW, ShowWindow, GetWindowPlacement};
use winapi::um::winuser::{SetCursorPos, SetWindowPos, UpdateWindow};
use winapi::um::winuser::{ClientToScreen, AttachThreadInput, ClipCursor};
use winapi::um::winuser::{GetClientRect, GetMenu, GetWindowLongA, GetWindowThreadProcessId};
use winapi::um::winuser::{AdjustWindowRectEx, GetWindowRect, SetWindowTextW};
use winapi::um::winnt::{LPCWSTR, LONG};
use winapi::um::processthreadsapi::GetCurrentThreadId;
use winapi::shared::minwindef::{BOOL, DWORD, UINT};
use winapi::shared::windef::{HDC, HWND, POINT};

use api::wgl::Context as WglContext;
use api::egl::Context as EglContext;
use api::egl::ffi::egl::Egl;

use self::init::RawContext;

mod callback;
mod event;
mod init;
mod monitor;

lazy_static! {
    static ref WAKEUP_MSG_ID: u32 = unsafe { RegisterWindowMessageA("Glutin::EventID".as_ptr() as *const i8) };
}

/// Cursor
pub type Cursor = *const wchar_t;

/// Contains information about states and the window for the callback.
#[derive(Clone)]
pub struct WindowState {
    pub cursor: Cursor,
    pub cursor_state: CursorState,
    pub attributes: WindowAttributes
}

/// The Win32 implementation of the main `Window` object.
pub struct Window {
    /// Main handle for the window.
    window: WindowWrapper,

    /// OpenGL context.
    context: Context,

    /// Receiver for the events dispatched by the window callback.
    events_receiver: Receiver<Event>,

    /// The current window state.
    window_state: Arc<Mutex<WindowState>>,
}

unsafe impl Send for Window {}
unsafe impl Sync for Window {}

enum Context {
    Egl(EglContext),
    Wgl(WglContext),
}

/// A simple wrapper that destroys the window when it is destroyed.
// FIXME: remove `pub` (https://github.com/rust-lang/rust/issues/23585)
#[doc(hidden)]
pub struct WindowWrapper(pub HWND, pub HDC);

impl Drop for WindowWrapper {
    #[inline]
    fn drop(&mut self) {
        unsafe {
            DestroyWindow(self.0);
        }
    }
}

#[derive(Clone)]
pub struct WindowProxy {
    hwnd: HWND,
}

unsafe impl Send for WindowProxy {}
unsafe impl Sync for WindowProxy {}

impl WindowProxy {
    #[inline]
    pub fn wakeup_event_loop(&self) {
        unsafe {
            PostMessageA(self.hwnd, *WAKEUP_MSG_ID, 0, 0);
        }
    }
}

impl Window {
    /// See the docs in the crate root file.
    pub fn new(window: &WindowAttributes, pf_reqs: &PixelFormatRequirements,
               opengl: &GlAttributes<&Window>, egl: Option<&Egl>)
               -> Result<Window, CreationError>
    {
        let opengl = opengl.clone().map_sharing(|sharing| {
            match sharing.context {
                Context::Wgl(ref c) => RawContext::Wgl(c.get_hglrc()),
                Context::Egl(_) => unimplemented!(),        // FIXME:
            }
        });

        init::new_window(window, pf_reqs, &opengl, egl)
    }

    /// See the docs in the crate root file.
    ///
    /// Calls SetWindowText on the HWND.
    pub fn set_title(&self, text: &str) {
        let text = OsStr::new(text).encode_wide().chain(Some(0).into_iter())
                                   .collect::<Vec<_>>();

        unsafe {
            SetWindowTextW(self.window.0, text.as_ptr() as LPCWSTR);
        }
    }

    #[inline]
    pub fn show(&self) {
        unsafe {
            ShowWindow(self.window.0, SW_SHOW);
        }
    }

    #[inline]
    pub fn hide(&self) {
        unsafe {
            ShowWindow(self.window.0, SW_HIDE);
        }
    }

    /// See the docs in the crate root file.
    pub fn get_position(&self) -> Option<(i32, i32)> {
        use std::mem;

        let mut placement: WINDOWPLACEMENT = unsafe { mem::zeroed() };
        placement.length = mem::size_of::<WINDOWPLACEMENT>() as UINT;

        if unsafe { GetWindowPlacement(self.window.0, &mut placement) } == 0 {
            return None
        }

        let ref rect = placement.rcNormalPosition;
        Some((rect.left as i32, rect.top as i32))
    }

    /// See the docs in the crate root file.
    pub fn set_position(&self, x: i32, y: i32) {
        use libc;

        unsafe {
            SetWindowPos(self.window.0, ptr::null_mut(), x as libc::c_int, y as libc::c_int,
                0, 0, SWP_NOZORDER | SWP_NOSIZE);
            UpdateWindow(self.window.0);
        }
    }

    /// See the docs in the crate root file.
    #[inline]
    pub fn get_inner_size(&self) -> Option<(u32, u32)> {
        let mut rect: RECT = unsafe { mem::uninitialized() };

        if unsafe { GetClientRect(self.window.0, &mut rect) } == 0 {
            return None
        }

        Some((
            (rect.right - rect.left) as u32,
            (rect.bottom - rect.top) as u32
        ))
    }

    /// See the docs in the crate root file.
    #[inline]
    pub fn get_outer_size(&self) -> Option<(u32, u32)> {
        let mut rect: RECT = unsafe { mem::uninitialized() };

        if unsafe { GetWindowRect(self.window.0, &mut rect) } == 0 {
            return None
        }

        Some((
            (rect.right - rect.left) as u32,
            (rect.bottom - rect.top) as u32
        ))
    }

    /// See the docs in the crate root file.
    pub fn set_inner_size(&self, x: u32, y: u32) {
        use libc;

        unsafe {
            // Calculate the outer size based upon the specified inner size
            let mut rect = RECT { top: 0, left: 0, bottom: y as LONG, right: x as LONG };
            let dw_style = GetWindowLongA(self.window.0, GWL_STYLE) as DWORD;
            let b_menu = !GetMenu(self.window.0).is_null() as BOOL;
            let dw_style_ex = GetWindowLongA(self.window.0, GWL_EXSTYLE) as DWORD;
            AdjustWindowRectEx(&mut rect, dw_style, b_menu, dw_style_ex);
            let outer_x = (rect.right - rect.left).abs() as libc::c_int;
            let outer_y = (rect.top - rect.bottom).abs() as libc::c_int;

            SetWindowPos(self.window.0, ptr::null_mut(), 0, 0, outer_x, outer_y,
                SWP_NOZORDER | SWP_NOREPOSITION | SWP_NOMOVE);
            UpdateWindow(self.window.0);
        }
    }

    #[inline]
    pub fn create_window_proxy(&self) -> WindowProxy {
        WindowProxy { hwnd: self.window.0 }
    }

    /// See the docs in the crate root file.
    #[inline]
    pub fn poll_events(&self) -> PollEventsIterator {
        PollEventsIterator {
            window: self,
        }
    }

    /// See the docs in the crate root file.
    #[inline]
    pub fn wait_events(&self) -> WaitEventsIterator {
        WaitEventsIterator {
            window: self,
        }
    }

    #[inline]
    pub fn platform_display(&self) -> *mut libc::c_void {
        // What should this return on win32?
        // It could be GetDC(NULL), but that requires a ReleaseDC()
        // to avoid leaking the DC.
        ptr::null_mut()
    }

    #[inline]
    pub fn platform_window(&self) -> *mut libc::c_void {
        self.window.0 as *mut libc::c_void
    }

    #[inline]
    pub fn set_window_resize_callback(&mut self, _: Option<fn(u32, u32)>) {
    }

    #[inline]
    pub fn set_cursor(&self, _cursor: MouseCursor) {
        let cursor_id = match _cursor {
            MouseCursor::Arrow | MouseCursor::Default => IDC_ARROW,
            MouseCursor::Hand => IDC_HAND,
            MouseCursor::Crosshair => IDC_CROSS,
            MouseCursor::Text | MouseCursor::VerticalText => IDC_IBEAM,
            MouseCursor::NotAllowed | MouseCursor::NoDrop => IDC_NO,
            MouseCursor::EResize => IDC_SIZEWE,
            MouseCursor::NResize => IDC_SIZENS,
            MouseCursor::WResize => IDC_SIZEWE,
            MouseCursor::SResize => IDC_SIZENS,
            MouseCursor::EwResize | MouseCursor::ColResize => IDC_SIZEWE,
            MouseCursor::NsResize | MouseCursor::RowResize => IDC_SIZENS,
            MouseCursor::Wait | MouseCursor::Progress => IDC_WAIT,
            MouseCursor::Help => IDC_HELP,
            _ => IDC_ARROW, // use arrow for the missing cases.
        };

        let mut cur = self.window_state.lock().unwrap();
        cur.cursor = cursor_id;
    }


    pub fn set_cursor_state(&self, state: CursorState) -> Result<(), String> {
        let mut current_state = self.window_state.lock().unwrap();

        let foreground_thread_id = unsafe { GetWindowThreadProcessId(self.window.0, ptr::null_mut()) };
        let current_thread_id = unsafe { GetCurrentThreadId() };

        unsafe { AttachThreadInput(foreground_thread_id, current_thread_id, 1) };

        let res = match (state, current_state.cursor_state) {
            (CursorState::Normal, CursorState::Normal) => Ok(()),
            (CursorState::Hide, CursorState::Hide) => Ok(()),
            (CursorState::Grab, CursorState::Grab) => Ok(()),

            (CursorState::Hide, CursorState::Normal) => {
                current_state.cursor_state = CursorState::Hide;
                Ok(())
            },

            (CursorState::Normal, CursorState::Hide) => {
                current_state.cursor_state = CursorState::Normal;
                Ok(())
            },

            (CursorState::Grab, CursorState::Normal) | (CursorState::Grab, CursorState::Hide) => {
                unsafe {
                    let mut rect = mem::uninitialized();
                    if GetClientRect(self.window.0, &mut rect) == 0 {
                        return Err(format!("GetWindowRect failed"));
                    }
                    ClientToScreen(self.window.0, mem::transmute(&mut rect.left));
                    ClientToScreen(self.window.0, mem::transmute(&mut rect.right));
                    if ClipCursor(&rect) == 0 {
                        return Err(format!("ClipCursor failed"));
                    }
                    current_state.cursor_state = CursorState::Grab;
                    Ok(())
                }
            },

            (CursorState::Normal, CursorState::Grab) => {
                unsafe {
                    if ClipCursor(ptr::null()) == 0 {
                        return Err(format!("ClipCursor failed"));
                    }
                    current_state.cursor_state = CursorState::Normal;
                    Ok(())
                }
            },

            _ => unimplemented!(),
        };

        unsafe { AttachThreadInput(foreground_thread_id, current_thread_id, 0) };

        res
    }

    #[inline]
    pub fn hidpi_factor(&self) -> f32 {
        1.0
    }

    pub fn set_cursor_position(&self, x: i32, y: i32) -> Result<(), ()> {
        let mut point = POINT {
            x: x,
            y: y,
        };

        unsafe {
            if ClientToScreen(self.window.0, &mut point) == 0 {
                return Err(());
            }

            if SetCursorPos(point.x, point.y) == 0 {
                return Err(());
            }
        }

        Ok(())
    }
}

impl GlContext for Window {
    #[inline]
    unsafe fn make_current(&self) -> Result<(), ContextError> {
        match self.context {
            Context::Wgl(ref c) => c.make_current(),
            Context::Egl(ref c) => c.make_current(),
        }
    }

    #[inline]
    fn is_current(&self) -> bool {
        match self.context {
            Context::Wgl(ref c) => c.is_current(),
            Context::Egl(ref c) => c.is_current(),
        }
    }

    #[inline]
    fn get_proc_address(&self, addr: &str) -> *const () {
        match self.context {
            Context::Wgl(ref c) => c.get_proc_address(addr),
            Context::Egl(ref c) => c.get_proc_address(addr),
        }
    }

    #[inline]
    fn swap_buffers(&self) -> Result<(), ContextError> {
        match self.context {
            Context::Wgl(ref c) => c.swap_buffers(),
            Context::Egl(ref c) => c.swap_buffers(),
        }
    }

    #[inline]
    fn get_api(&self) -> Api {
        match self.context {
            Context::Wgl(ref c) => c.get_api(),
            Context::Egl(ref c) => c.get_api(),
        }
    }

    #[inline]
    fn get_pixel_format(&self) -> PixelFormat {
        match self.context {
            Context::Wgl(ref c) => c.get_pixel_format(),
            Context::Egl(ref c) => c.get_pixel_format(),
        }
    }
}

pub struct PollEventsIterator<'a> {
    window: &'a Window,
}

impl<'a> Iterator for PollEventsIterator<'a> {
    type Item = Event;

    #[inline]
    fn next(&mut self) -> Option<Event> {
        self.window.events_receiver.try_recv().ok()
    }
}

pub struct WaitEventsIterator<'a> {
    window: &'a Window,
}

impl<'a> Iterator for WaitEventsIterator<'a> {
    type Item = Event;

    #[inline]
    fn next(&mut self) -> Option<Event> {
        self.window.events_receiver.recv().ok()
    }
}

impl Drop for Window {
    #[inline]
    fn drop(&mut self) {
        unsafe {
            // we don't call MakeCurrent(0, 0) because we are not sure that the context
            // is still the current one
            PostMessageW(self.window.0, WM_DESTROY, 0, 0);
        }
    }
}
