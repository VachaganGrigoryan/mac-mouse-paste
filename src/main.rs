mod engine;

use cocoa::appkit::{
    NSApp, NSApplication, NSApplicationActivationPolicyAccessory, NSMenu, NSMenuItem, NSStatusBar,
    NSVariableStatusItemLength,
};
use cocoa::base::{id, nil};
use cocoa::foundation::{NSAutoreleasePool, NSString};
use engine::Engine;
use objc::declare::ClassDecl;
use objc::runtime::{Class, Object, Sel};
use objc::{class, msg_send, sel, sel_impl};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::OnceLock;

// -------- Engine singleton --------
static ENGINE: OnceLock<Engine> = OnceLock::new();
fn engine() -> &'static Engine {
    ENGINE.get_or_init(Engine::new)
}

// -------- UI pointers to update state --------
// NOTE: storing ObjC pointers as usize is common in Rust Cocoa glue.
// These are valid for app lifetime since menu items are retained by NSMenu.
static STATUS_ITEM_PTR: AtomicUsize = AtomicUsize::new(0);
static RUN_ITEM_PTR: AtomicUsize = AtomicUsize::new(0);
static STATUSBAR_BUTTON_PTR: AtomicUsize = AtomicUsize::new(0);

unsafe fn set_enabled(item: id, enabled: bool) {
    let _: () = msg_send![item, setEnabled: if enabled { 1i32 } else { 0i32 }];
}

unsafe fn set_title(item: id, title: &str) {
    let s = NSString::alloc(nil).init_str(title);
    let _: () = msg_send![item, setTitle: s];
}

unsafe fn set_state(item: id, on: bool) {
    let _: () = msg_send![item, setState: if on { 1i32 } else { 0i32 }];
}

unsafe fn refresh_menu_state() {
    let running = engine().is_running();

    // Status line
    if let Some(st_item) = (STATUS_ITEM_PTR.load(Ordering::SeqCst) as usize).checked_into_id() {
        set_title(st_item, if running { "Status: Running" } else { "Status: Stopped" });
        set_state(st_item, running);
    }

    // Single Start/Stop item label
    if let Some(run_item) = (RUN_ITEM_PTR.load(Ordering::SeqCst) as usize).checked_into_id() {
        set_title(run_item, if running { "Stop" } else { "Start" });
    }

    // Menu bar icon
    if let Some(btn) = (STATUSBAR_BUTTON_PTR.load(Ordering::SeqCst) as usize).checked_into_id() {
        let icon = if running { "📋✅" } else { "📋⏸" };
        let t = NSString::alloc(nil).init_str(icon);
        let _: () = msg_send![btn, setTitle: t];
    }
}

/// Helper trait to turn stored usize back into ObjC id safely-ish.
trait UsizeToId {
    unsafe fn checked_into_id(self) -> Option<id>;
}
impl UsizeToId for usize {
    unsafe fn checked_into_id(self) -> Option<id> {
        if self == 0 {
            None
        } else {
            Some(self as id)
        }
    }
}

// -------- Objective-C handler --------
fn create_delegate_class() -> *const Class {
    static mut CLS: *const Class = std::ptr::null();
    static ONCE: std::sync::Once = std::sync::Once::new();

    ONCE.call_once(|| unsafe {
        let superclass = class!(NSObject);
        let mut decl = ClassDecl::new("RustMenuHandler", superclass).unwrap();

        extern "C" fn noop(_this: &Object, _cmd: Sel, _sender: id) {}

        extern "C" fn on_quit(_this: &Object, _cmd: Sel, _sender: id) {
            engine().stop();
            unsafe {
                let app: id = NSApp();
                let _: () = msg_send![app, terminate: nil];
            }
        }

        extern "C" fn on_tick(_this: &Object, _cmd: Sel, _sender: id) {
            unsafe { refresh_menu_state(); }
        }

        extern "C" fn on_toggle_run(_this: &Object, _cmd: Sel, _sender: id) {
            if engine().is_running() {
                engine().stop();
            } else {
                engine().start(false);
            }
            unsafe { refresh_menu_state(); }
        }

        decl.add_method(sel!(noop:), noop as extern "C" fn(&Object, Sel, id));
        decl.add_method(sel!(onToggleRun:), on_toggle_run as extern "C" fn(&Object, Sel, id));
        decl.add_method(sel!(onQuit:), on_quit as extern "C" fn(&Object, Sel, id));
        decl.add_method(sel!(onTick:), on_tick as extern "C" fn(&Object, Sel, id));

        CLS = decl.register();
    });

    unsafe { CLS }
}

fn main() {
    unsafe {
        let _pool = NSAutoreleasePool::new(nil);

        let app = NSApp();
        app.setActivationPolicy_(NSApplicationActivationPolicyAccessory);

        let handler_cls = create_delegate_class();
        let handler: id = msg_send![handler_cls, new];

        // Create Status Bar Item
        let status_bar = NSStatusBar::systemStatusBar(nil);
        let status_item = status_bar.statusItemWithLength_(NSVariableStatusItemLength);

        // Store button pointer so we can change icon
        let button: id = msg_send![status_item, button];
        STATUSBAR_BUTTON_PTR.store(button as usize, Ordering::SeqCst);

        // Initial icon
        let title = NSString::alloc(nil).init_str("📋⏸");
        let _: () = msg_send![button, setTitle: title];

        // Create Menu
        let menu = NSMenu::new(nil).autorelease();

        // Status line (disabled)
        let status_title = NSString::alloc(nil).init_str("Status: …");
        let status_item_line = NSMenuItem::alloc(nil)
            .initWithTitle_action_keyEquivalent_(status_title, sel!(noop:), NSString::alloc(nil).init_str(""))
            .autorelease();
        let _: () = msg_send![status_item_line, setEnabled: 0i32];
        menu.addItem_(status_item_line);
        STATUS_ITEM_PTR.store(status_item_line as usize, Ordering::SeqCst);

        menu.addItem_(NSMenuItem::separatorItem(nil));

        let run_title = NSString::alloc(nil).init_str("Start"); // will be updated by refresh
        let run_item = NSMenuItem::alloc(nil)
            .initWithTitle_action_keyEquivalent_(
                run_title,
                sel!(onToggleRun:),
                NSString::alloc(nil).init_str(""),
            )
            .autorelease();

        let _: () = msg_send![run_item, setTarget: handler];
        menu.addItem_(run_item);

        RUN_ITEM_PTR.store(run_item as usize, Ordering::SeqCst);

        menu.addItem_(NSMenuItem::separatorItem(nil));

        // Quit
        let quit_title = NSString::alloc(nil).init_str("Quit");
        let quit_item = NSMenuItem::alloc(nil)
            .initWithTitle_action_keyEquivalent_(
                quit_title,
                sel!(onQuit:),
                NSString::alloc(nil).init_str("q"),
            )
            .autorelease();
        let _: () = msg_send![quit_item, setTarget: handler];
        menu.addItem_(quit_item);

        // Attach menu to status item
        let _: () = msg_send![status_item, setMenu: menu];

        // Timer to refresh state even if engine dies
        let _: id = msg_send![class!(NSTimer),
            scheduledTimerWithTimeInterval: 1.0
            target: handler
            selector: sel!(onTick:)
            userInfo: nil
            repeats: 1
        ];

        // Initial refresh (sets icon + enabled/disabled items)
        refresh_menu_state();

        // If you want auto-start engine on app launch, uncomment:
        // engine().start(false);

        app.run();
    }
}
