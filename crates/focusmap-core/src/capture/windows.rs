//! Windows input capture: two low-level hooks plus a foreground-app poller.
//!
//! Privacy: the keyboard hook looks at `vkCode` for exactly one comparison
//! (is this the backspace key?) and then drops it. Nothing about the key, and
//! nothing about any window title, leaves this module.

use std::sync::atomic::{AtomicBool, AtomicI32, AtomicU32, AtomicU64, Ordering};
use std::sync::mpsc::{sync_channel, SyncSender};
use std::sync::{Arc, OnceLock};
use std::thread::JoinHandle;

use anyhow::{bail, Result};
use windows_sys::Win32::Foundation::CloseHandle;
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::System::SystemInformation::GetTickCount64;
use windows_sys::Win32::System::Threading::{
    GetCurrentThreadId, OpenProcess, QueryFullProcessImageNameW, PROCESS_QUERY_LIMITED_INFORMATION,
};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{GetLastInputInfo, LASTINPUTINFO, VK_BACK};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, DispatchMessageW, GetForegroundWindow, GetMessageW, GetWindowThreadProcessId,
    PostThreadMessageW, SetWindowsHookExW, TranslateMessage, UnhookWindowsHookEx,
    KBDLLHOOKSTRUCT, MSG, MSLLHOOKSTRUCT, WH_KEYBOARD_LL, WH_MOUSE_LL, WM_KEYDOWN,
    WM_LBUTTONDOWN, WM_MBUTTONDOWN, WM_MOUSEMOVE, WM_MOUSEWHEEL, WM_RBUTTONDOWN, WM_SYSKEYDOWN,
    WM_XBUTTONDOWN,
};

use super::{now_ms, CaptureEvent, InputSource};

/// Mouse moves are batched before they reach the channel.
const MOVE_EMIT_MS: u64 = 60;

static SINK: OnceLock<SyncSender<CaptureEvent>> = OnceLock::new();
static PAUSED: AtomicBool = AtomicBool::new(false);
static HOOK_THREAD_ID: AtomicU32 = AtomicU32::new(0);
static LAST_X: AtomicI32 = AtomicI32::new(i32::MIN);
static LAST_Y: AtomicI32 = AtomicI32::new(i32::MIN);
static PENDING_PX: AtomicU64 = AtomicU64::new(0);
static PENDING_EVENTS: AtomicU32 = AtomicU32::new(0);
static LAST_MOVE_EMIT: AtomicU64 = AtomicU64::new(0);

/// Stop emitting events without removing the hooks (pause / focus mode).
pub fn set_paused(paused: bool) {
    PAUSED.store(paused, Ordering::Relaxed);
}

pub fn is_paused() -> bool {
    PAUSED.load(Ordering::Relaxed)
}

fn emit(event: CaptureEvent) {
    if PAUSED.load(Ordering::Relaxed) {
        return;
    }
    if let Some(sink) = SINK.get() {
        let _ = sink.try_send(event);
    }
}

unsafe extern "system" fn keyboard_proc(code: i32, wparam: usize, lparam: isize) -> isize {
    if code == 0 {
        let msg = wparam as u32;
        if msg == WM_KEYDOWN || msg == WM_SYSKEYDOWN {
            let info = &*(lparam as *const KBDLLHOOKSTRUCT);
            // The key value is used here and nowhere else. It is never stored,
            // sent or logged.
            let backspace = info.vkCode == VK_BACK as u32;
            emit(CaptureEvent::Key {
                ts: now_ms(),
                backspace,
            });
        }
    }
    CallNextHookEx(std::ptr::null_mut(), code, wparam, lparam)
}

unsafe extern "system" fn mouse_proc(code: i32, wparam: usize, lparam: isize) -> isize {
    if code == 0 {
        let msg = wparam as u32;
        match msg {
            WM_MOUSEMOVE => {
                let info = &*(lparam as *const MSLLHOOKSTRUCT);
                let (x, y) = (info.pt.x, info.pt.y);
                let last_x = LAST_X.swap(x, Ordering::Relaxed);
                let last_y = LAST_Y.swap(y, Ordering::Relaxed);
                if last_x != i32::MIN {
                    let dx = (x - last_x).unsigned_abs() as u64;
                    let dy = (y - last_y).unsigned_abs() as u64;
                    PENDING_PX.fetch_add(dx + dy, Ordering::Relaxed);
                    PENDING_EVENTS.fetch_add(1, Ordering::Relaxed);
                }
                let now = GetTickCount64();
                let last = LAST_MOVE_EMIT.load(Ordering::Relaxed);
                if now.saturating_sub(last) >= MOVE_EMIT_MS {
                    LAST_MOVE_EMIT.store(now, Ordering::Relaxed);
                    let px = PENDING_PX.swap(0, Ordering::Relaxed).min(u32::MAX as u64) as u32;
                    let events = PENDING_EVENTS.swap(0, Ordering::Relaxed);
                    if events > 0 {
                        emit(CaptureEvent::Move {
                            ts: now_ms(),
                            px,
                            events,
                        });
                    }
                }
            }
            WM_LBUTTONDOWN | WM_RBUTTONDOWN | WM_MBUTTONDOWN | WM_XBUTTONDOWN | WM_MOUSEWHEEL => {
                emit(CaptureEvent::Click { ts: now_ms() });
            }
            _ => {}
        }
    }
    CallNextHookEx(std::ptr::null_mut(), code, wparam, lparam)
}

#[derive(Default)]
pub struct WindowsSource {
    running: Option<Arc<AtomicBool>>,
    hook_thread: Option<JoinHandle<()>>,
    app_thread: Option<JoinHandle<()>>,
}

impl InputSource for WindowsSource {
    fn start(&mut self, sink: SyncSender<CaptureEvent>) -> Result<()> {
        let _ = SINK.set(sink);
        let running = Arc::new(AtomicBool::new(true));
        let (ready_tx, ready_rx) = sync_channel::<Result<()>>(1);

        let hook_running = running.clone();
        self.hook_thread = Some(std::thread::spawn(move || {
            let module = unsafe { GetModuleHandleW(std::ptr::null()) };
            let keyboard =
                unsafe { SetWindowsHookExW(WH_KEYBOARD_LL, Some(keyboard_proc), module, 0) };
            if keyboard.is_null() {
                let _ = ready_tx.send(Err(anyhow::anyhow!(
                    "Windows refused the keyboard hook (error {})",
                    std::io::Error::last_os_error()
                )));
                return;
            }
            let mouse = unsafe { SetWindowsHookExW(WH_MOUSE_LL, Some(mouse_proc), module, 0) };
            if mouse.is_null() {
                unsafe { UnhookWindowsHookEx(keyboard) };
                let _ = ready_tx.send(Err(anyhow::anyhow!(
                    "Windows refused the mouse hook (error {})",
                    std::io::Error::last_os_error()
                )));
                return;
            }
            HOOK_THREAD_ID.store(unsafe { GetCurrentThreadId() }, Ordering::Relaxed);
            let _ = ready_tx.send(Ok(()));

            let mut msg = MSG::default();
            while hook_running.load(Ordering::Relaxed) {
                let result = unsafe { GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) };
                if result <= 0 {
                    break;
                }
                unsafe {
                    TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }
            }
            unsafe {
                UnhookWindowsHookEx(keyboard);
                UnhookWindowsHookEx(mouse);
            }
        }));

        match ready_rx.recv_timeout(std::time::Duration::from_secs(5)) {
            Ok(Ok(())) => {}
            Ok(Err(err)) => return Err(err),
            Err(_) => bail!("input hooks did not start in time"),
        }

        let poll_running = running.clone();
        self.app_thread = Some(std::thread::spawn(move || {
            let mut last_app = String::new();
            let mut last_idle = u32::MAX;
            while poll_running.load(Ordering::Relaxed) {
                if let Some(name) = unsafe { foreground_app() } {
                    if name != last_app {
                        emit(CaptureEvent::App {
                            ts: now_ms(),
                            name: name.clone(),
                        });
                        last_app = name;
                    }
                }
                let idle = idle_seconds();
                if idle != last_idle {
                    emit(CaptureEvent::Idle {
                        ts: now_ms(),
                        secs: idle,
                    });
                    last_idle = idle;
                }
                // 1s poll: cheap enough to sit inside the 1% CPU budget.
                for _ in 0..10 {
                    if !poll_running.load(Ordering::Relaxed) {
                        break;
                    }
                    std::thread::sleep(std::time::Duration::from_millis(100));
                }
            }
        }));
        self.running = Some(running);
        Ok(())
    }

    fn stop(&mut self) {
        if let Some(running) = self.running.take() {
            running.store(false, Ordering::Relaxed);
        }
        let thread_id = HOOK_THREAD_ID.swap(0, Ordering::Relaxed);
        if thread_id != 0 {
            unsafe {
                PostThreadMessageW(thread_id, 0x0012 /* WM_QUIT */, 0, 0);
            }
        }
        if let Some(handle) = self.hook_thread.take() {
            let _ = handle.join();
        }
        if let Some(handle) = self.app_thread.take() {
            let _ = handle.join();
        }
    }

    fn label(&self) -> &'static str {
        "Windows hooks"
    }
}

impl Drop for WindowsSource {
    fn drop(&mut self) {
        self.stop();
    }
}

unsafe fn foreground_app() -> Option<String> {
    let hwnd = GetForegroundWindow();
    if hwnd.is_null() {
        return None;
    }
    let mut pid = 0u32;
    GetWindowThreadProcessId(hwnd, &mut pid);
    if pid == 0 {
        return None;
    }
    let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
    if handle.is_null() {
        return None;
    }
    let mut buf = [0u16; 512];
    let mut len = buf.len() as u32;
    let ok = QueryFullProcessImageNameW(handle, 0, buf.as_mut_ptr(), &mut len);
    CloseHandle(handle);
    if ok == 0 || len == 0 {
        return None;
    }
    let path = String::from_utf16_lossy(&buf[..len as usize]);
    path.rsplit(['\\', '/'])
        .next()
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
}

fn idle_seconds() -> u32 {
    let mut info = LASTINPUTINFO {
        cbSize: std::mem::size_of::<LASTINPUTINFO>() as u32,
        dwTime: 0,
    };
    let ok = unsafe { GetLastInputInfo(&mut info) };
    if ok == 0 {
        return 0;
    }
    let now = unsafe { GetTickCount64() };
    ((now.saturating_sub(info.dwTime as u64)) / 1000) as u32
}
