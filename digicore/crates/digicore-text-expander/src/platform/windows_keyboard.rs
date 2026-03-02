//! Windows low-level keyboard hook with proper message loop.
//!
//! Replaces rdev::listen which only processes one message then exits.
//! Uses SetWindowsHookExW(WH_KEYBOARD_LL) + message loop.
//! Creates a hidden window so the thread has a proper message queue.

use std::ffi::c_int;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Input::KeyboardAndMouse::*;
use windows::Win32::UI::WindowsAndMessaging::*;

static HOOK_ACTIVE: AtomicBool = AtomicBool::new(false);

/// Key event: (virtual_key_code, character_from_ToUnicode_or_None) -> true to consume key (block from app)
pub type KeyCallback = Box<dyn Fn(u16, Option<char>) -> bool + Send>;

/// Start keyboard listener in background thread. Runs until process exits.
pub fn start_listener(callback: KeyCallback) -> anyhow::Result<()> {
    if HOOK_ACTIVE.swap(true, Ordering::SeqCst) {
        return Ok(());
    }

    let callback = Arc::new(std::sync::Mutex::new(callback));

    thread::spawn(move || {
        let _ = run_hook(callback);
        HOOK_ACTIVE.store(false, Ordering::SeqCst);
    });

    Ok(())
}

pub fn is_listener_running() -> bool {
    HOOK_ACTIVE.load(Ordering::SeqCst)
}

fn run_hook(callback: Arc<std::sync::Mutex<KeyCallback>>) -> anyhow::Result<()> {
    unsafe {
        let hook = SetWindowsHookExW(
            WINDOWS_HOOK_ID(WH_KEYBOARD_LL.0),
            Some(hook_proc),
            GetModuleHandleW(None)?,
            0,
        )
        .map_err(|e| anyhow::anyhow!("SetWindowsHookExW: {:?}", e))?;

        // Store hook handle and callback for the hook procedure
        HOOK_DATA.with(|cell| {
            *cell.borrow_mut() = Some(HookData {
                hook,
                callback: callback.clone(),
            });
        });

        // Message loop - PeekMessage + Sleep (GetMessage can block forever in worker threads)
        let mut msg = MSG::default();
        loop {
            while PeekMessageW(&mut msg, HWND::default(), 0, 0, PM_REMOVE).as_bool() {
                if msg.message == WM_QUIT {
                    UnhookWindowsHookEx(hook)?;
                    HOOK_DATA.with(|cell| *cell.borrow_mut() = None);
                    return Ok(());
                }
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
            thread::sleep(Duration::from_millis(10));
        }
    }
}

struct HookData {
    hook: HHOOK,
    callback: Arc<std::sync::Mutex<KeyCallback>>,
}

thread_local! {
    static HOOK_DATA: std::cell::RefCell<Option<HookData>> = std::cell::RefCell::new(None);
}

unsafe extern "system" fn hook_proc(code: c_int, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    let hhook = HOOK_DATA.with(|cell| cell.borrow().as_ref().map(|d| d.hook));
    let hhook = hhook.unwrap_or(HHOOK::default());

    const HC_ACTION: c_int = 0;
    if code != HC_ACTION {
        return CallNextHookEx(hhook, code, wparam, lparam);
    }

    const WM_KEYDOWN: usize = 0x0100;
    const WM_SYSKEYDOWN: usize = 0x0104;
    let wparam_val = wparam.0;
    if wparam_val != WM_KEYDOWN && wparam_val != WM_SYSKEYDOWN {
        return CallNextHookEx(hhook, code, wparam, lparam);
    }

    let kb = &*(lparam.0 as *const KBDLLHOOKSTRUCT);
    let vk_code = kb.vkCode as u16;
    let scan_code = kb.scanCode as u16;

    let ch = get_char_from_vk(vk_code, scan_code, lparam);

    let consume = HOOK_DATA.with(|cell| {
        if let Some(ref data) = *cell.borrow() {
            if let Ok(cb) = data.callback.lock() {
                return cb(vk_code, ch);
            }
        }
        false
    });

    if consume {
        return LRESULT(1);
    }
    CallNextHookEx(hhook, code, wparam, lparam)
}

const VK_SHIFT: u16 = 0x10;
const VK_CONTROL: u16 = 0x11;

/// Check if Ctrl key is currently pressed (for Ghost Suggestor Ctrl+Tab).
pub fn is_ctrl_pressed() -> bool {
    unsafe { (GetAsyncKeyState(VK_CONTROL as i32) as u16 & 0x8000) != 0 }
}

/// Get character from virtual key using ToUnicodeEx (respects keyboard layout).
unsafe fn get_char_from_vk(vk_code: u16, scan_code: u16, _lparam: LPARAM) -> Option<char> {
    let shift = (GetAsyncKeyState(VK_SHIFT as i32) as u16 & 0x8000) != 0;

    let mut state = [0u8; 256];
    if GetKeyboardState(&mut state).is_err() {
        return vk_to_char_fallback(vk_code, shift);
    }

    let hwnd = GetForegroundWindow();
    let thread_id = GetWindowThreadProcessId(hwnd, None);
    let layout = GetKeyboardLayout(thread_id);

    let mut buf = [0u16; 8];
    if shift {
        state[VK_SHIFT as usize] |= 0x80;
    }
    let len = ToUnicodeEx(
        vk_code as u32,
        scan_code as u32,
        &state,
        &mut buf,
        0,
        layout,
    );

    match len {
        1 => char::from_u32(buf[0] as u32),
        _ => vk_to_char_fallback(vk_code, shift),
    }
}

/// Fallback when ToUnicodeEx fails (e.g. dead key, or no char).
/// shift: true if Shift key is pressed.
fn vk_to_char_fallback(vk: u16, shift: bool) -> Option<char> {
    Some(match vk {
        0x20 => ' ',
        0x30 => if shift { ')' } else { '0' },
        0x31 => if shift { '!' } else { '1' },
        0x32 => if shift { '@' } else { '2' },
        0x33 => if shift { '#' } else { '3' },
        0x34 => if shift { '$' } else { '4' },
        0x35 => if shift { '%' } else { '5' },
        0x36 => if shift { '^' } else { '6' },
        0x37 => if shift { '&' } else { '7' },
        0x38 => if shift { '*' } else { '8' },
        0x39 => if shift { '(' } else { '9' },
        0x41..=0x5A => {
            if shift {
                vk as u8 as char
            } else {
                (vk as u8 + 32) as char
            }
        }
        0xBA => if shift { ':' } else { ';' },
        0xBB => if shift { '+' } else { '=' },
        0xBC => if shift { '<' } else { ',' },
        0xBD => if shift { '_' } else { '-' },
        0xBE => if shift { '>' } else { '.' },
        0xBF => if shift { '?' } else { '/' },
        0xC0 => if shift { '~' } else { '`' },
        0xDB => if shift { '{' } else { '[' },
        0xDC => if shift { '|' } else { '\\' },
        0xDD => if shift { '}' } else { ']' },
        0xDE => if shift { '"' } else { '\'' },
        _ => return None,
    })
}
