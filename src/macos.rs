//! Finder sends open-document Apple events, not command-line arguments. Keep a handler alive
//! for both a cold launch and files opened while the Rune event loop is already running.
use objc2::{
    ClassType, class, msg_send,
    rc::autoreleasepool,
    runtime::{AnyClass, AnyObject, ClassBuilder, NSObject, Sel},
    sel,
};
use objc2_foundation::{NSString, NSURL};
use std::{
    path::PathBuf,
    sync::{Arc, OnceLock},
};

type OpenFiles = Arc<dyn Fn(Vec<PathBuf>) + Send + Sync>;
static OPEN_FILES: OnceLock<OpenFiles> = OnceLock::new();
const CORE: u32 = u32::from_be_bytes(*b"aevt");
const OPEN: u32 = u32::from_be_bytes(*b"odoc");
const DIRECT: u32 = u32::from_be_bytes(*b"----");

extern "C" fn open_documents(_: &AnyObject, _: Sel, event: *mut AnyObject, _: *mut AnyObject) {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        autoreleasepool(|_| unsafe {
            let Some(event) = event.as_ref() else { return };
            let list: *mut AnyObject = msg_send![event, paramDescriptorForKeyword: DIRECT];
            let Some(list) = list.as_ref() else { return };
            let count: isize = msg_send![list, numberOfItems];
            let mut paths = Vec::new();
            for i in 1..=count {
                let desc: *mut AnyObject = msg_send![list, descriptorAtIndex: i];
                // NSURL can resolve Finder aliases and file URL descriptors without lossy path parsing.
                let url: *mut NSURL = msg_send![desc, fileURLValue];
                if let Some(url) = url.as_ref() {
                    let path: *mut NSString = msg_send![url, path];
                    if let Some(path) = path.as_ref() {
                        paths.push(PathBuf::from(path.to_string()));
                    }
                }
            }
            if !paths.is_empty()
                && let Some(open) = OPEN_FILES.get()
            {
                open(paths);
            }
        });
    }));
    if result.is_err() {
        eprintln!("Folio: couldn’t handle open-document event");
    }
}

extern "C" fn launching(handler: &AnyObject, _: Sel, _: *mut AnyObject) {
    // AppKit sets up its document dispatcher during launch, after NSApplication is made.
    // WillFinishLaunching is the documented point to override an Apple-event handler.
    unsafe {
        let manager: *mut AnyObject =
            msg_send![class!(NSAppleEventManager), sharedAppleEventManager];
        let _: () = msg_send![manager, setEventHandler: handler, andSelector: sel!(openDocuments:reply:), forEventClass: CORE, andEventID: OPEN];
    }
}

pub fn reduce_transparency() -> bool {
    autoreleasepool(|_| unsafe {
        let workspace: *mut AnyObject = msg_send![class!(NSWorkspace), sharedWorkspace];
        let reduce: objc2::runtime::Bool =
            msg_send![workspace, accessibilityDisplayShouldReduceTransparency];
        reduce.as_bool()
    })
}

pub fn reveal_file(path: &std::path::Path) {
    // Called on the UI thread after a palette/menu action; no shell interpolation.
    autoreleasepool(|_| unsafe {
        let path = NSString::from_str(&path.to_string_lossy());
        let url: *mut AnyObject = msg_send![class!(NSURL), fileURLWithPath: &*path];
        let urls: *mut AnyObject = msg_send![class!(NSArray), arrayWithObject: url];
        let workspace: *mut AnyObject = msg_send![class!(NSWorkspace), sharedWorkspace];
        let _: () = msg_send![workspace, activateFileViewerSelectingURLs: urls];
    });
}

pub fn install_open_handler(open: impl Fn(Vec<PathBuf>) + Send + Sync + 'static) {
    let _ = OPEN_FILES.set(Arc::new(open));
    // NSObject and NSAppleEventManager are only accessed on the application's main thread.
    autoreleasepool(|_| unsafe {
        // NSApplication installs its standard odoc handler during initialization. Create it
        // first, then replace that handler, or the cold-launch file event is swallowed.
        let _: *mut AnyObject = msg_send![class!(NSApplication), sharedApplication];
        let class = AnyClass::get("FolioOpenDocuments").unwrap_or_else(|| {
            let mut builder = ClassBuilder::new("FolioOpenDocuments", NSObject::class())
                .expect("unique handler class");
            builder.add_method(
                sel!(openDocuments:reply:),
                open_documents as extern "C" fn(_, _, _, _),
            );
            builder.add_method(sel!(launching:), launching as extern "C" fn(_, _, _));
            builder.register()
        });
        let handler: *mut AnyObject = msg_send![class, new];
        // Both managers borrow this +1 reference for the process lifetime.
        let center: *mut AnyObject = msg_send![class!(NSNotificationCenter), defaultCenter];
        for name in [
            "NSApplicationWillFinishLaunchingNotification",
            "NSApplicationDidFinishLaunchingNotification",
        ] {
            let name = NSString::from_str(name);
            let _: () = msg_send![center, addObserver: handler, selector: sel!(launching:), name: &*name, object: std::ptr::null::<AnyObject>()];
        }
        launching(&*handler, sel!(launching:), std::ptr::null_mut());
    });
}
