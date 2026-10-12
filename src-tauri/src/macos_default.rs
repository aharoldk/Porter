//! macOS default-browser registration through LaunchServices.
//!
//! Porter must be installed as `Porter.app` whose `Info.plist` declares the
//! `http`/`https` URL schemes (see `plugins > deep-link > desktop > schemes`
//! in `tauri.conf.json`) for `LSSetDefaultHandlerForURLScheme` to accept it.
#![cfg(target_os = "macos")]

use std::ffi::CString;
use std::os::raw::{c_char, c_void};

pub const BUNDLE_ID: &str = "dev.porter.app";

const K_CF_STRING_ENCODING_UTF8: u32 = 0x0800_0100;

#[link(name = "CoreFoundation", kind = "framework")]
extern "C" {
    fn CFStringCreateWithCString(
        allocator: *const c_void,
        c_str: *const c_char,
        encoding: u32,
    ) -> *const c_void;
    fn CFStringGetCString(
        the_string: *const c_void,
        buffer: *mut c_char,
        buffer_size: isize,
        encoding: u32,
    ) -> bool;
    fn CFRelease(cf: *const c_void);
}

#[link(name = "CoreServices", kind = "framework")]
extern "C" {
    fn LSSetDefaultHandlerForURLScheme(scheme: *const c_void, bundle_id: *const c_void) -> i32;
    fn LSCopyDefaultHandlerForURLScheme(scheme: *const c_void) -> *const c_void;
}

fn cf_string(value: &str) -> *const c_void {
    let c_string =
        CString::new(value).expect("bundle id and URL schemes never contain interior nul bytes");
    unsafe { CFStringCreateWithCString(std::ptr::null(), c_string.as_ptr(), K_CF_STRING_ENCODING_UTF8) }
}

fn string_from_cf(cf: *const c_void) -> Option<String> {
    if cf.is_null() {
        return None;
    }
    let mut buffer = [0 as c_char; 512];
    let ok = unsafe {
        CFStringGetCString(
            cf,
            buffer.as_mut_ptr(),
            buffer.len() as isize,
            K_CF_STRING_ENCODING_UTF8,
        )
    };
    unsafe { CFRelease(cf) };
    if !ok {
        return None;
    }
    let bytes: Vec<u8> = buffer
        .iter()
        .take_while(|&&byte| byte != 0)
        .map(|&byte| byte as u8)
        .collect();
    String::from_utf8(bytes).ok()
}

/// Registers Porter as the default handler for the given URL scheme.
pub fn set_default(scheme: &str) -> Result<(), String> {
    let scheme_ref = cf_string(scheme);
    let bundle_ref = cf_string(BUNDLE_ID);
    let status = unsafe { LSSetDefaultHandlerForURLScheme(scheme_ref, bundle_ref) };
    unsafe {
        CFRelease(scheme_ref);
        CFRelease(bundle_ref);
    }
    if status == 0 {
        Ok(())
    } else {
        Err(format!(
            "LaunchServices refused to set Porter as the {scheme} handler (error {status}). \
             Porter must be installed as Porter.app."
        ))
    }
}

/// Returns the bundle id currently handling the given URL scheme.
pub fn current_default(scheme: &str) -> Option<String> {
    let scheme_ref = cf_string(scheme);
    let current = unsafe { LSCopyDefaultHandlerForURLScheme(scheme_ref) };
    unsafe { CFRelease(scheme_ref) };
    string_from_cf(current)
}
