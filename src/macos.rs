use std::num::NonZeroUsize;

use accessibility_ng::{AXAttribute, AXUIElement};
use accessibility_sys_ng::{kAXFocusedUIElementAttribute, kAXSelectedTextAttribute};
use active_win_pos_rs::get_active_window;
use core_foundation::string::CFString;
use debug_print::debug_println;
use lru::LruCache;
use parking_lot::Mutex;
use crate::utils::*{
    get_selected_text_by_clipboard,
    get_context_via_select_all,
};
use crate::GetTextError;
use enigo::{Enigo, Keyboard, Settings};
use std::{thread, time::Duration};

static GET_SELECTED_TEXT_METHOD: Mutex<Option<LruCache<String, u8>>> = Mutex::new(None);

pub fn get_selected_text() -> Result<String, Box<dyn std::error::Error>> {
    if GET_SELECTED_TEXT_METHOD.lock().is_none() {
        let cache = LruCache::new(NonZeroUsize::new(100).unwrap());
        *GET_SELECTED_TEXT_METHOD.lock() = Some(cache);
    }
    let mut cache = GET_SELECTED_TEXT_METHOD.lock();
    let cache = cache.as_mut().unwrap();
    let app_name = match get_active_window() {
        Ok(window) => window.app_name,
        Err(_) => return Err("No active window found".into()),
    };
    // debug_println!("app_name: {}", app_name);
    if let Some(text) = cache.get(&app_name) {
        if *text == 0 {
            return get_selected_text_by_ax();
        }
        return get_selected_text_by_clipboard_using_applescript();
    }
    match get_selected_text_by_ax() {
        Ok(text) => {
            if !text.is_empty() {
                cache.put(app_name, 0);
            }
            Ok(text)
        }
        Err(_) => match get_selected_text_by_clipboard_using_applescript() {
            Ok(text) => {
                if !text.is_empty() {
                    cache.put(app_name, 1);
                }
                Ok(text)
            }
            Err(e) => Err(e),
        },
    }
}

fn get_selected_text_by_ax() -> Result<String, Box<dyn std::error::Error>> {
    // debug_println!("get_selected_text_by_ax");
    let system_element = AXUIElement::system_wide();
    let Some(selected_element) = system_element
        .attribute(&AXAttribute::new(&CFString::from_static_string(
            kAXFocusedUIElementAttribute,
        )))
        .map(|element| element.downcast_into::<AXUIElement>())
        .ok()
        .flatten()
    else {
        return Err(Box::new(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "No selected element",
        )));
    };
    let Some(selected_text) = selected_element
        .attribute(&AXAttribute::new(&CFString::from_static_string(
            kAXSelectedTextAttribute,
        )))
        .map(|text| text.downcast_into::<CFString>())
        .ok()
        .flatten()
    else {
        return Err(Box::new(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "No selected text",
        )));
    };
    Ok(selected_text.to_string())
}

const APPLE_SCRIPT: &str = r#"
use AppleScript version "2.4"
use scripting additions
use framework "Foundation"
use framework "AppKit"

set savedAlertVolume to alert volume of (get volume settings)

-- Back up clipboard contents:
set savedClipboard to the clipboard

set thePasteboard to current application's NSPasteboard's generalPasteboard()
set theCount to thePasteboard's changeCount()

tell application "System Events"
    set volume alert volume 0
end tell

-- Copy selected text to clipboard:
tell application "System Events" to keystroke "c" using {command down}
delay 0.1 -- Without this, the clipboard may have stale data.

tell application "System Events"
    set volume alert volume savedAlertVolume
end tell

if thePasteboard's changeCount() is theCount then
    return ""
end if

set theSelectedText to the clipboard

set the clipboard to savedClipboard

theSelectedText
"#;

fn get_selected_text_by_clipboard_using_applescript() -> Result<String, Box<dyn std::error::Error>>
{
    // debug_println!("get_selected_text_by_clipboard_using_applescript");
    let output = std::process::Command::new("osascript")
        .arg("-e")
        .arg(APPLE_SCRIPT)
        .output()?;
    if output.status.success() {
        let content = String::from_utf8(output.stdout)?;
        let content = content.trim();
        Ok(content.to_string())
    } else {
        let err = output
            .stderr
            .into_iter()
            .map(|c| c as char)
            .collect::<String>()
            .into();
        Err(err)
    }
}

pub fn get_selected_text_os(cancel_select: bool) -> Result<String, GetTextError> {
    let mut enigo = Enigo::new(&Settings::default()).map_err(|e| GetTextError::Input(e.to_string()))?;
    get_selected_text_by_clipboard(&mut enigo, cancel_select)
}

pub fn get_selected_text_with_context_os(
    cancel_select: bool,
) -> Result<(String, Option<String>), GetTextError> {
    let mut enigo = Enigo::new(&Settings::default()).map_err(|e| GetTextError::Input(e.to_string()))?;

    // 1. Get selected text using standard clipboard method first
    let selected_text = get_selected_text_by_clipboard(&mut enigo, cancel_select)?;

    if selected_text.is_empty() {
        // If no text was selected, we can't get context
        return Ok((selected_text, None));
    }

    // 2. On macOS, directly use the fallback: Select All + Copy
    // Short delay before fallback simulation
    thread::sleep(Duration::from_millis(100));
    match get_context_via_select_all(&mut enigo, &selected_text) {
        Ok(Some(context)) => Ok((selected_text, Some(context))),
        Ok(None)=> Ok((selected_text, None)), // Should not happen
        Err(GetTextError::NotInContext) => {
            eprintln!("Fallback failed: Selected text not found in full text.");
            Ok((selected_text, None)) 
        }
        Err(e) => {
            eprintln!("Fallback context retrieval failed: {}", e);
            Ok((selected_text, None))
        }
    }
}
