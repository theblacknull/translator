// Phím Alt toàn hệ thống: nhấn và thả Alt một mình thì sao chép chữ đang chọn và dịch.
// Dùng hook bàn phím cấp thấp của Windows (crate `windows`, thuần Rust).
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use windows::Win32::Foundation::{HINSTANCE, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, VIRTUAL_KEY, VK_C,
    VK_CONTROL, VK_LMENU, VK_MENU, VK_RMENU,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, GetMessageW, SetWindowsHookExW, KBDLLHOOKSTRUCT, MSG, WH_KEYBOARD_LL, WM_KEYDOWN,
    WM_KEYUP, WM_SYSKEYDOWN, WM_SYSKEYUP,
};

static ALT_DOWN: AtomicBool = AtomicBool::new(false);
static DIRTY: AtomicBool = AtomicBool::new(false);
type Callback = Box<dyn Fn() + Send + Sync>;
static ON_ALT: OnceLock<Callback> = OnceLock::new();

const VK_NONAME: u16 = 0xFC; // phím trống: hủy chế độ menu khi thả Alt

unsafe extern "system" fn hook_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code >= 0 {
        let msg = wparam.0 as u32;
        let info = unsafe { &*(lparam.0 as *const KBDLLHOOKSTRUCT) };
        let vk = info.vkCode as u16;
        let down = msg == WM_KEYDOWN || msg == WM_SYSKEYDOWN;
        let up = msg == WM_KEYUP || msg == WM_SYSKEYUP;
        let is_alt = vk == VK_MENU.0 || vk == VK_LMENU.0 || vk == VK_RMENU.0;

        if is_alt {
            if down && !ALT_DOWN.load(Ordering::SeqCst) {
                ALT_DOWN.store(true, Ordering::SeqCst);
                DIRTY.store(false, Ordering::SeqCst);
            } else if up && ALT_DOWN.load(Ordering::SeqCst) {
                ALT_DOWN.store(false, Ordering::SeqCst);
                if !DIRTY.load(Ordering::SeqCst) {
                    // Thả Alt sẽ mở thanh menu và làm Ctrl+C bị bỏ qua; chèn phím trống để hủy
                    send_key(VK_NONAME, false);
                    send_key(VK_NONAME, true);
                    if let Some(cb) = ON_ALT.get() {
                        cb();
                    }
                }
            }
        } else if down && ALT_DOWN.load(Ordering::SeqCst) {
            // Giữ Alt mà nhấn phím khác (Alt+Tab...) thì không phải lệnh dịch
            DIRTY.store(true, Ordering::SeqCst);
        }
    }
    CallNextHookEx(None, code, wparam, lparam)
}

fn send_key(vk: u16, up: bool) {
    let input = INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: VIRTUAL_KEY(vk),
                wScan: 0,
                dwFlags: if up { KEYEVENTF_KEYUP } else { Default::default() },
                time: 0,
                dwExtraInfo: 0,
            },
        },
    };
    unsafe {
        SendInput(&[input], std::mem::size_of::<INPUT>() as i32);
    }
}

/// Gửi Ctrl+C vào cửa sổ đang có chữ được chọn
pub fn send_ctrl_c() {
    send_key(VK_CONTROL.0, false);
    send_key(VK_C.0, false);
    send_key(VK_C.0, true);
    send_key(VK_CONTROL.0, true);
}

/// Lấy chữ đang chọn: đặt mốc vào clipboard, sao chép, rồi đọc lại
pub fn grab_selection() -> Option<String> {
    let mut cb = arboard::Clipboard::new().ok()?;
    let probe = format!(
        "__probe_{}",
        SystemTime::now().duration_since(UNIX_EPOCH).ok()?.as_nanos()
    );
    cb.set_text(probe.clone()).ok()?;
    send_ctrl_c();
    for _ in 0..60 {
        thread::sleep(Duration::from_millis(12));
        if let Ok(text) = cb.get_text() {
            if text != probe {
                let t = text.trim().to_string();
                return if t.is_empty() { None } else { Some(t) };
            }
        }
    }
    None
}

/// Bắt đầu hook trên một luồng riêng; mỗi lần nhấn-thả Alt sẽ gọi `on_alt`
pub fn start(on_alt: impl Fn() + Send + Sync + 'static) {
    let _ = ON_ALT.set(Box::new(on_alt));
    thread::spawn(|| unsafe {
        let hmod = GetModuleHandleW(None).map(|m| HINSTANCE(m.0)).unwrap_or(HINSTANCE(std::ptr::null_mut()));
        let hook = SetWindowsHookExW(WH_KEYBOARD_LL, Some(hook_proc), hmod, 0);
        if hook.is_err() {
            return;
        }
        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {}
    });
}
