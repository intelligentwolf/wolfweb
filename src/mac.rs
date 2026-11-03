//! macOS application plumbing: CEF requires NSApp to be an NSApplication that implements its CefAppProtocol, and
//! Chromium must shut down by leaving the run loop rather than through -terminate:'s exit().
//! Source: cef-rs examples/cefsimple/src/mac/mod.rs (SimpleApplication / SimpleAppDelegate), with WolfWeb's
//! window handling in place of cefsimple's SimpleHandler.

use cef::application_mac::{CefAppProtocol, CrAppControlProtocol, CrAppProtocol};
use objc2::{
    define_class, extern_methods, msg_send,
    rc::Retained,
    runtime::{AnyObject, Bool, NSObject, NSObjectProtocol, ProtocolObject},
    sel, ClassType, DefinedClass, MainThreadMarker, MainThreadOnly,
};
use objc2_app_kit::{
    NSApp, NSApplication, NSApplicationDelegate, NSApplicationTerminateReply, NSEvent, NSUserInterfaceValidations,
    NSValidatedUserInterfaceItem,
};
use objc2_foundation::{ns_string, NSBundle, NSObjectNSThreadPerformAdditions};
use std::{cell::Cell, ptr};

define_class! {
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    pub struct WolfAppDelegate;

    impl WolfAppDelegate {
        /// Create the application menu (MainMenu.nib, compiled from resources/mac/English.lproj/MainMenu.xib).
        #[unsafe(method(createApplication:))]
        unsafe fn create_application(&self, _object: Option<&AnyObject>) {
            let app = NSApp(MainThreadMarker::new().expect("Not running on the main thread"));
            assert!(app.isKindOfClass(WolfApplication::class()));
            assert!(app.delegate().unwrap().isKindOfClass(WolfAppDelegate::class()));

            let main_bundle = NSBundle::mainBundle();
            let _: Bool = msg_send![&main_bundle,
                loadNibNamed: ns_string!("MainMenu"),
                owner: &*app,
                topLevelObjects: ptr::null_mut::<*const AnyObject>()
            ];
        }
    }

    unsafe impl NSObjectProtocol for WolfAppDelegate {}

    unsafe impl NSApplicationDelegate for WolfAppDelegate {
        #[unsafe(method(applicationShouldTerminate:))]
        unsafe fn application_should_terminate(&self, _sender: &NSApplication) -> NSApplicationTerminateReply {
            NSApplicationTerminateReply::TerminateNow
        }

        /// Dock icon clicked while WolfWeb runs: open a window if none is open.
        #[unsafe(method(applicationShouldHandleReopen:hasVisibleWindows:))]
        unsafe fn application_should_handle_reopen(&self, _sender: &NSApplication, has_visible_windows: Bool) -> Bool {
            if !has_visible_windows.as_bool() && crate::tabs::browsers().is_empty() {
                crate::tabs::open_window_with_tabs(Vec::new());
            }
            Bool::NO
        }

        /// Secure state-restoration archives (macOS 12+) — cefsimple, see https://crrev.com/c737387656.
        #[unsafe(method(applicationSupportsSecureRestorableState:))]
        unsafe fn application_supports_secure_restorable_state(&self, _sender: &NSApplication) -> Bool {
            Bool::YES
        }
    }

    unsafe impl NSUserInterfaceValidations for WolfAppDelegate {
        #[unsafe(method(validateUserInterfaceItem:))]
        unsafe fn validate_user_interface_item(&self, item: &ProtocolObject<dyn NSValidatedUserInterfaceItem>) -> Bool {
            // Source: cef_command_ids.h — IDC_FIND 37000 (as in cefsimple).
            const IDC_FIND: isize = 37000;
            if item.tag() == IDC_FIND { Bool::YES } else { Bool::NO }
        }
    }
}

impl WolfAppDelegate {
    fn new(mtm: MainThreadMarker) -> Retained<Self> {
        let this = WolfAppDelegate::alloc(mtm).set_ivars(());
        unsafe { msg_send![super(this), init] }
    }
}

#[derive(Default)]
pub struct WolfApplicationIvars {
    handling_send_event: Cell<Bool>,
}

define_class!(
    /// An NSApplication subclass implementing the protocols CEF requires (cefsimple SimpleApplication).
    #[unsafe(super(NSApplication))]
    #[ivars = WolfApplicationIvars]
    pub struct WolfApplication;

    impl WolfApplication {
        #[unsafe(method(sendEvent:))]
        unsafe fn send_event(&self, event: &NSEvent) {
            let was_sending_event = self.is_handling_send_event();
            if !was_sending_event {
                self.set_handling_send_event(true);
            }
            let _: () = msg_send![super(self), sendEvent:event];
            if !was_sending_event {
                self.set_handling_send_event(false);
            }
        }

        /// Quit (menu, Dock, logout): close every browser; the last one closing quits the message loop
        /// (tabs::on_browser_closed), which is the orderly shutdown Chromium needs. The default -terminate: calls
        /// exit() and never leaves the run loop (cefsimple's comment on SimpleApplication terminate:).
        #[unsafe(method(terminate:))]
        unsafe fn terminate(&self, _sender: &AnyObject) {
            crate::tabs::close_all_browsers();
        }
    }

    unsafe impl CrAppControlProtocol for WolfApplication {
        #[unsafe(method(setHandlingSendEvent:))]
        unsafe fn _set_handling_send_event(&self, handling_send_event: Bool) {
            self.ivars().handling_send_event.set(handling_send_event);
        }
    }

    unsafe impl CrAppProtocol for WolfApplication {
        #[unsafe(method(isHandlingSendEvent))]
        unsafe fn _is_handling_send_event(&self) -> Bool {
            self.ivars().handling_send_event.get()
        }
    }

    unsafe impl CefAppProtocol for WolfApplication {}
);

impl WolfApplication {
    extern_methods! {
        #[unsafe(method(sharedApplication))]
        fn shared_application() -> Retained<Self>;

        #[unsafe(method(setHandlingSendEvent:))]
        fn set_handling_send_event(&self, handling_send_event: bool);

        #[unsafe(method(isHandlingSendEvent))]
        fn is_handling_send_event(&self) -> bool;
    }
}

/// Make NSApp a WolfApplication. Must run before anything else touches NSApp.
pub fn setup_application() {
    let _ = WolfApplication::shared_application();
    assert!(
        NSApp(MainThreadMarker::new().expect("Not running on the main thread")).isKindOfClass(WolfApplication::class()),
        "NSApp was created before WolfWeb's NSApplication subclass"
    );
}

/// Install the app delegate and build the menu (after CEF initialize()).
pub fn setup_app_delegate() -> Retained<WolfAppDelegate> {
    let mtm = MainThreadMarker::new().expect("Not running on the main thread");
    let delegate = WolfAppDelegate::new(mtm);
    let proto = ProtocolObject::<dyn NSApplicationDelegate>::from_retained(delegate.clone());
    let app = NSApp(mtm);
    assert!(app.isKindOfClass(WolfApplication::class()));
    app.setDelegate(Some(&proto));
    unsafe {
        delegate.performSelectorOnMainThread_withObject_waitUntilDone(sel!(createApplication:), None, false);
    }
    delegate
}
