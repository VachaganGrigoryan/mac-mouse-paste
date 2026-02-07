use core_foundation::mach_port::{CFMachPortCreateRunLoopSource, CFMachPortRef};
use core_foundation::runloop::{
    kCFRunLoopDefaultMode, CFRunLoopAddSource, CFRunLoopGetCurrent, CFRunLoopRun, CFRunLoopRef,
};
use core_graphics::event::{
    CGEvent, CGEventFlags, CGEventMask, CGEventTapLocation, CGEventTapProxy, CGEventType,
    CGMouseButton,
};
use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};
use core_graphics::geometry::CGPoint;
use core_graphics::sys::CGEventRef;
use foreign_types_shared::ForeignType;
use libc::{c_void, usleep};
use std::io::Write;
use std::process::{Command, Stdio};
use std::ptr;
use std::sync::atomic::{AtomicBool, AtomicI64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{SystemTime, UNIX_EPOCH};

pub struct Engine {
    state: Mutex<State>,
    running: Arc<AtomicBool>,
}

struct State {
    thread: Option<JoinHandle<()>>,
}

impl Engine {
    pub fn new() -> Self {
        Self {
            state: Mutex::new(State {
                thread: None,
            }),
            running: Arc::new(AtomicBool::new(false)),
        }
    }


    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }

    pub fn start(&self, dont_paste: bool) {
        if self.is_running() {
            return;
        }
        self.running.store(true, Ordering::SeqCst);

        let running = self.running.clone();

        let mut st = self.state.lock().unwrap();
        st.thread = Some(std::thread::spawn(move || {
            // Run the loop. If it fails immediately, we still clear running below.
            run_event_tap_loop(dont_paste);

            // IMPORTANT: if we exit (permissions failure, tap disabled, etc),
            // reflect that Start is not actually running.
            running.store(false, Ordering::SeqCst);
        }));
    }

    pub fn stop(&self) {
        // Stop the runloop if present
        let ptr = RUNLOOP_PTR.load(Ordering::SeqCst);
        if ptr != 0 {
            let rl = ptr as CFRunLoopRef;
            unsafe {
                CFRunLoopStop(rl);
                CFRunLoopWakeUp(rl);
            }
        }

        let mut st = self.state.lock().unwrap();
        if let Some(handle) = st.thread.take() {
            let _ = handle.join();
        }

        self.running.store(false, Ordering::SeqCst);

        RUNLOOP_PTR.store(0, Ordering::SeqCst);
    }
}

/* ---------- global runloop handle to stop ---------- */

static RUNLOOP_PTR: AtomicUsize = AtomicUsize::new(0);


#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    fn CFRunLoopStop(rl: CFRunLoopRef);
    fn CFRunLoopWakeUp(rl: CFRunLoopRef);
}

/* -----------------------------
   Your “Linux-like primary” buffer
   ----------------------------- */

static PRIMARY_BUF: Mutex<Option<String>> = Mutex::new(None);
static LOCK_UNTIL_PASTE: AtomicBool = AtomicBool::new(true);

fn pbpaste() -> Option<String> {
    let out = Command::new("/usr/bin/pbpaste").output().ok()?;
    if !out.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&out.stdout).to_string())
}

fn pbcopy(s: &str) -> bool {
    let mut child = match Command::new("/usr/bin/pbcopy")
        .stdin(Stdio::piped())
        .spawn()
    {
        Ok(c) => c,
        Err(_) => return false,
    };

    if let Some(stdin) = child.stdin.as_mut() {
        if stdin.write_all(s.as_bytes()).is_err() {
            let _ = child.kill();
            return false;
        }
    }
    child.wait().map(|st| st.success()).unwrap_or(false)
}

/* -----------------------------
   Click state
   ----------------------------- */

const DOUBLE_CLICK_MILLIS: i64 = 500;
static IS_DRAGGING: AtomicBool = AtomicBool::new(false);
static PREV_CLICK_MS: AtomicI64 = AtomicI64::new(0);
static CUR_CLICK_MS: AtomicI64 = AtomicI64::new(0);

fn now_millis() -> i64 {
    let dur = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    (dur.as_secs() as i64) * 1000 + (dur.subsec_millis() as i64)
}

fn record_click_time() {
    let cur = now_millis();
    let prev = CUR_CLICK_MS.swap(cur, Ordering::SeqCst);
    PREV_CLICK_MS.store(prev, Ordering::SeqCst);
}

fn is_double_click() -> bool {
    let prev = PREV_CLICK_MS.load(Ordering::SeqCst);
    let cur = CUR_CLICK_MS.load(Ordering::SeqCst);
    (cur - prev) < DOUBLE_CLICK_MILLIS
}

/* -----------------------------
   Synth input helpers
   ----------------------------- */

fn send_cmd_key(keycode: u16) {
    let source = CGEventSource::new(CGEventSourceStateID::CombinedSessionState)
        .expect("Failed to create CGEventSource");

    let down = CGEvent::new_keyboard_event(source.clone(), keycode, true)
        .expect("Failed to create key down");
    let up = CGEvent::new_keyboard_event(source, keycode, false)
        .expect("Failed to create key up");

    down.set_flags(CGEventFlags::CGEventFlagCommand);
    down.post(CGEventTapLocation::AnnotatedSession);
    up.post(CGEventTapLocation::AnnotatedSession);
}

fn copy_cmd_c() {
    const VK_ANSI_C: u16 = 0x08;
    send_cmd_key(VK_ANSI_C);
}

fn paste_cmd_v() {
    const VK_ANSI_V: u16 = 0x09;
    send_cmd_key(VK_ANSI_V);
}

fn focus_click_at(event: &CGEvent) {
    let loc: CGPoint = event.location();

    let source = CGEventSource::new(CGEventSourceStateID::CombinedSessionState)
        .expect("Failed to create CGEventSource");

    let down = CGEvent::new_mouse_event(
        source.clone(),
        CGEventType::LeftMouseDown,
        loc,
        CGMouseButton::Left,
    )
        .expect("Failed to create mouse down");

    let up = CGEvent::new_mouse_event(
        source,
        CGEventType::LeftMouseUp,
        loc,
        CGMouseButton::Left,
    )
        .expect("Failed to create mouse up");

    down.post(CGEventTapLocation::HID);
    up.post(CGEventTapLocation::HID);
}

fn capture_primary_selection_locked() {
    if LOCK_UNTIL_PASTE.load(Ordering::SeqCst) && PRIMARY_BUF.lock().unwrap().is_some() {
        return;
    }

    let clipboard_before = pbpaste().unwrap_or_default();

    copy_cmd_c();
    unsafe { usleep(20_000) };

    let copied = pbpaste().unwrap_or_default();
    let _ = pbcopy(&clipboard_before);

    if copied.trim().is_empty() {
        return;
    }

    *PRIMARY_BUF.lock().unwrap() = Some(copied);
}

fn paste_primary(event: &CGEvent) {
    let Some(text) = PRIMARY_BUF.lock().unwrap().clone() else { return };

    let clipboard_before = pbpaste().unwrap_or_default();
    let _ = pbcopy(&text);

    focus_click_at(event);
    unsafe { usleep(20_000) };
    paste_cmd_v();

    unsafe { usleep(20_000) };
    let _ = pbcopy(&clipboard_before);

    *PRIMARY_BUF.lock().unwrap() = None;
}

/* -----------------------------
   Quartz Event Tap FFI
   ----------------------------- */

#[repr(C)]
struct CallbackCtx {
    dont_paste: bool,
}

type CGEventTapLocationRaw = u32;
type CGEventTapPlacementRaw = u32;
type CGEventTapOptionsRaw = u32;

const K_CG_SESSION_EVENT_TAP: CGEventTapLocationRaw = 1;
const K_CG_TAIL_APPEND_EVENT_TAP: CGEventTapPlacementRaw = 1;
const K_CG_EVENT_TAP_OPTION_LISTEN_ONLY: CGEventTapOptionsRaw = 1;

type CGEventTapCallBack = extern "C" fn(
    proxy: CGEventTapProxy,
    type_: CGEventType,
    event: CGEventRef,
    user_info: *mut c_void,
) -> CGEventRef;

#[link(name = "ApplicationServices", kind = "framework")]
unsafe extern "C" {
    fn CGEventTapCreate(
        tap: CGEventTapLocationRaw,
        place: CGEventTapPlacementRaw,
        options: CGEventTapOptionsRaw,
        events_of_interest: CGEventMask,
        callback: CGEventTapCallBack,
        user_info: *mut c_void,
    ) -> CFMachPortRef;
}

extern "C" fn mouse_callback(
    _proxy: CGEventTapProxy,
    type_: CGEventType,
    event: CGEventRef,
    user_info: *mut c_void,
) -> CGEventRef {
    if event.is_null() {
        return event;
    }

    let ctx = unsafe { &*(user_info as *const CallbackCtx) };
    let ev = unsafe { CGEvent::from_ptr(event) }; // NOT owned -> must forget

    match type_ {
        CGEventType::OtherMouseDown => {
            if !ctx.dont_paste {
                paste_primary(&ev);
            }
        }
        CGEventType::LeftMouseDown => record_click_time(),
        CGEventType::LeftMouseDragged => IS_DRAGGING.store(true, Ordering::SeqCst),
        CGEventType::LeftMouseUp => {
            let dragging = IS_DRAGGING.load(Ordering::SeqCst);
            if is_double_click() || dragging {
                capture_primary_selection_locked();
            }
            IS_DRAGGING.store(false, Ordering::SeqCst);
        }
        _ => {}
    }

    std::mem::forget(ev);
    event
}

fn cg_event_mask_bit(t: CGEventType) -> CGEventMask {
    1u64 << (t as u64)
}

fn run_event_tap_loop(dont_paste: bool) {
    // Save runloop ref so we can stop it later
    let rl = unsafe { CFRunLoopGetCurrent() };
    RUNLOOP_PTR.store(rl as usize, Ordering::SeqCst);

    let mut ctx = Box::new(CallbackCtx { dont_paste });

    let mask: CGEventMask = cg_event_mask_bit(CGEventType::OtherMouseDown)
        | cg_event_mask_bit(CGEventType::LeftMouseDown)
        | cg_event_mask_bit(CGEventType::LeftMouseUp)
        | cg_event_mask_bit(CGEventType::LeftMouseDragged);

    let tap: CFMachPortRef = unsafe {
        CGEventTapCreate(
            K_CG_SESSION_EVENT_TAP,
            K_CG_TAIL_APPEND_EVENT_TAP,
            K_CG_EVENT_TAP_OPTION_LISTEN_ONLY,
            mask,
            mouse_callback,
            (&mut *ctx as *mut CallbackCtx) as *mut c_void,
        )
    };

    if tap.is_null() {
        eprintln!("Failed to create event tap. Check Accessibility/Input Monitoring.");
        return;
    }

    let source = unsafe { CFMachPortCreateRunLoopSource(ptr::null(), tap, 0) };
    unsafe {
        CFRunLoopAddSource(rl, source, kCFRunLoopDefaultMode);
    }

    // keep ctx alive for callback lifetime
    std::mem::forget(ctx);

    unsafe { CFRunLoopRun() };
}
