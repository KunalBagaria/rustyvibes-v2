//! Launch at login through `SMAppService.mainAppService` (macOS 13+).

use objc2::msg_send;
use objc2::rc::Retained;
use objc2::runtime::{AnyClass, AnyObject};
use objc2_foundation::NSError;

#[link(name = "ServiceManagement", kind = "framework")]
unsafe extern "C" {}

/// `SMAppServiceStatus` values.
const ENABLED: isize = 1;
const REQUIRES_APPROVAL: isize = 2;

fn service() -> Option<Retained<AnyObject>> {
    let class = AnyClass::get(c"SMAppService")?;
    // SAFETY: `+[SMAppService mainAppService]` returns a non-nil object.
    Some(unsafe { msg_send![class, mainAppService] })
}

fn status() -> isize {
    // SAFETY: `-[SMAppService status]` returns an NSInteger-backed enum.
    service().map_or(0, |s| unsafe { msg_send![&*s, status] })
}

pub fn is_enabled() -> bool {
    status() == ENABLED
}

pub fn requires_approval() -> bool {
    status() == REQUIRES_APPROVAL
}

pub fn set_enabled(on: bool) -> Result<(), String> {
    let service = service().ok_or("launch at login needs macOS 13 or later")?;
    // SAFETY: both methods take a trailing NSError** and return BOOL.
    let result: Result<(), Retained<NSError>> = unsafe {
        if on {
            msg_send![&*service, registerAndReturnError: _]
        } else {
            msg_send![&*service, unregisterAndReturnError: _]
        }
    };
    result.map_err(|e| e.localizedDescription().to_string())
}

/// Opens System Settings → General → Login Items.
pub fn open_login_items_settings() {
    if let Some(class) = AnyClass::get(c"SMAppService") {
        // SAFETY: class method without arguments or return value (macOS 13+).
        let _: () = unsafe { msg_send![class, openSystemSettingsLoginItems] };
    }
}
