use std::num::NonZeroUsize;

use accessibility_ng::{AXAttribute, AXUIElement, AXUIElementAttributes, AXValue};
use accessibility_sys_ng::{kAXFocusedUIElementAttribute, kAXSelectedTextAttribute};
use active_win_pos_rs::get_active_window;
use core_foundation::string::CFString;
use core_foundation::base::TCFType;
use core_foundation::number::CFNumber;
use core_foundation::boolean::CFBoolean;
use core_foundation::attributed_string::CFAttributedString;
use debug_print::debug_println;
use lru::LruCache;
use parking_lot::Mutex;
use enigo::{Enigo, Settings};

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
    if let Some(method_val) = cache.get(&app_name) {
        if *method_val == 0 {
            // Call the modified get_selected_text_by_ax and extract only the text
            return get_selected_text_by_ax().map(|(text, _context)| text);
        }
        return get_selected_text_by_clipboard_using_applescript();
    }

    match get_selected_text_by_ax() {
        Ok((text, _context)) => { // Adapt to new return type
            if !text.is_empty() {
                cache.put(app_name.clone(), 0);
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

fn get_selected_text_by_ax() -> Result<(String, Option<String>), Box<dyn std::error::Error>> {
    debug_println!("[AX_CONTEXT_LOG] Attempting to get selected text and context via AX API");
    let system_element = AXUIElement::system_wide();
    let focused_element = match system_element
        .attribute(&AXAttribute::new(&CFString::from_static_string(
            kAXFocusedUIElementAttribute,
        )))
        .map(|element| element.downcast_into::<AXUIElement>())
        .ok()
        .flatten()
    {
        Some(element) => element,
        None => {
            debug_println!("[AX_CONTEXT_LOG] No focused UI element found.");
            return Err(Box::new(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "No focused UI element",
            )));
        }
    };

    debug_println!("[AX_CONTEXT_LOG] Focused UI Element found. Logging attributes:");

    // Log various string attributes
    let string_attributes_to_log: [(&str, Box<dyn Fn() -> Result<CFString, accessibility_ng::Error>>); 11] = [
        ("Description", Box::new(|| focused_element.description())),
        ("Document", Box::new(|| focused_element.document())),
        ("Help", Box::new(|| focused_element.help())),
        ("Identifier", Box::new(|| focused_element.identifier())),
        ("LabelValue", Box::new(|| focused_element.label_value())),
        ("PlaceholderValue", Box::new(|| focused_element.placeholder_value())),
        ("Role", Box::new(|| focused_element.role())),
        ("RoleDescription", Box::new(|| focused_element.role_description())),
        ("Subrole", Box::new(|| focused_element.subrole())),
        ("Title", Box::new(|| focused_element.title())),
        ("ValueDescription", Box::new(|| focused_element.value_description())),
    ];


    for (name, getter) in &string_attributes_to_log {
        match getter() {
            Ok(value) => { debug_println!("[AX_CONTEXT_LOG]   Attribute [{}]: '{}'", name, value.to_string()) }
            Err(e) => { debug_println!("[AX_CONTEXT_LOG]   Error getting attribute [{}]: {:?}", name, e) }
        }
    }

    // Log AXValue (kAXValueAttribute)
    debug_println!("[AX_CONTEXT_LOG] Attempting to log AXValue (kAXValueAttribute):");
    match focused_element.value() {
        Ok(cf_type_val) => {
            if let Some(s) = cf_type_val.clone().downcast_into::<CFString>() {
                debug_println!("[AX_CONTEXT_LOG]   AXValue (as CFString): {}", s.to_string());
            } else if let Some(n) = cf_type_val.clone().downcast_into::<CFNumber>() {
                let num_val_i64 = n.to_i64();
                let num_val_f64 = n.to_f64();
                debug_println!("[AX_CONTEXT_LOG]   AXValue (as CFNumber): {:?} (i64: {:?}, f64: {:?})", n, num_val_i64, num_val_f64);
            }  else if let Some(ax_val_ref) = cf_type_val.clone().downcast_into::<AXValue>() { // Assuming AXValue is the correct type from accessibility_ng
                 debug_println!("[AX_CONTEXT_LOG]   AXValue (as AXValue of type {:?}): {:?}", ax_val_ref.type_of(), ax_val_ref);
            } 
        }
        Err(e) => { debug_println!("[AX_CONTEXT_LOG]   Error getting AXValue: {:?}", e) }
    }

    // Log NumberOfCharacters
    debug_println!("[AX_CONTEXT_LOG] Attempting to log NumberOfCharacters:");
    match focused_element.number_of_characters() {
        Ok(cf_number) => {
            let num_val = cf_number.to_i64().unwrap_or(-1);
            debug_println!("[AX_CONTEXT_LOG]   NumberOfCharacters: {} ({:?})", num_val, cf_number);
        },
        Err(e) => { debug_println!("[AX_CONTEXT_LOG]   Error getting NumberOfCharacters: {:?}", e) }
    }
    
    // Log VisibleCharacterRange and then AttributedStringForRange
    debug_println!("[AX_CONTEXT_LOG] Attempting to log VisibleCharacterRange and AttributedStringForRange:");
    match focused_element.visible_character_range() {
        Ok(range_ax_value) => {
            debug_println!("[AX_CONTEXT_LOG]   VisibleCharacterRange (AXValue): type {:?}, value {:?}", range_ax_value.type_of(), range_ax_value);
            // Now use this range to get the attributed string
            // The AXAttribute::attributed_string_for_range() comes from the accessibility_ng macro definitions
            
        }
        Err(e) => {
            debug_println!("[AX_CONTEXT_LOG]   Error getting VisibleCharacterRange: {:?}", e);
        }
    }

    // Original goal: Get selected text
    debug_println!("[AX_CONTEXT_LOG] Attempting to get kAXSelectedTextAttribute (primary goal):");
    let selected_text_string = match focused_element.attribute(&AXAttribute::new(&CFString::from_static_string(kAXSelectedTextAttribute))) {
        Ok(selected_text_cfvalue) => {
            if let Some(selected_text_cfstring) = selected_text_cfvalue.clone().downcast_into::<CFString>() {
                let text = selected_text_cfstring.to_string();
                debug_println!("[AX_CONTEXT_LOG]   Successfully retrieved kAXSelectedTextAttribute: '{}'", text);
                text
            } else {
                debug_println!("[AX_CONTEXT_LOG]   kAXSelectedTextAttribute was not a CFString. Type: {:?}", selected_text_cfvalue.type_of());
                return Err(Box::new(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "Selected text attribute was not a string",
                )));
            }
        }
        Err(e) => {
            debug_println!("[AX_CONTEXT_LOG]   Failed to get kAXSelectedTextAttribute: {:?}", e);
            return Err(Box::new(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("No selected text (AX error: {:?})", e),
            )));
        }
    };

    // Get description for context
    debug_println!("[AX_CONTEXT_LOG] Attempting to get context from both kAXValueAttribute and AXValue:");
    
    // Get context from kAXValueAttribute
    let value_context = match focused_element.value() {
        Ok(cf_type_val) => {
            if let Some(s) = cf_type_val.clone().downcast_into::<CFString>() {
                let text = s.to_string();
                debug_println!("[AX_CONTEXT_LOG]   AXValue (as CFString): '{}'", text);
                Some(text)
            } else if let Some(n) = cf_type_val.clone().downcast_into::<CFNumber>() {
                let num_val_i64 = n.to_i64();
                let num_val_f64 = n.to_f64();
                debug_println!("[AX_CONTEXT_LOG]   AXValue (as CFNumber): {:?} (i64: {:?}, f64: {:?})", n, num_val_i64, num_val_f64);
                None
            } else if let Some(ax_val_ref) = cf_type_val.clone().downcast_into::<AXValue>() {
                debug_println!("[AX_CONTEXT_LOG]   AXValue (as AXValue of type {:?}): {:?}", ax_val_ref.type_of(), ax_val_ref);
                None
            } else {
                None
            }
        }
        Err(e) => {
            debug_println!("[AX_CONTEXT_LOG]   Error getting AXValue: {:?}", e);
            None
        }
    };

    // Get context from kAXDescriptionAttribute
    let description_context = match focused_element.description() {
        Ok(cf_string) => {
            let desc_text = cf_string.to_string();
            debug_println!("[AX_CONTEXT_LOG]   kAXDescriptionAttribute: '{}'", desc_text);
            if desc_text.is_empty() { None } else { Some(desc_text) }
        }
        Err(e) => {
            debug_println!("[AX_CONTEXT_LOG]   Failed to get kAXDescriptionAttribute: {:?}", e);
            None
        }
    };

    // Compare and select the better context
    let description_string_option = match (value_context, description_context) {
        (Some(v), Some(d)) => {
            if v.len() > d.len() {
                debug_println!("[AX_CONTEXT_LOG]   Selected AXValue as context (length: {})", v.len());
                Some(v)
            } else {
                debug_println!("[AX_CONTEXT_LOG]   Selected kAXDescriptionAttribute as context (length: {})", d.len());
                Some(d)
            }
        }
        (Some(v), None) => {
            debug_println!("[AX_CONTEXT_LOG]   Selected AXValue as context (only option)");
            Some(v)
        }
        (None, Some(d)) => {
            debug_println!("[AX_CONTEXT_LOG]   Selected kAXDescriptionAttribute as context (only option)");
            Some(d)
        }
        (None, None) => {
            debug_println!("[AX_CONTEXT_LOG]   No context available from either source");
            None
        }
    };
    
    Ok((selected_text_string, description_string_option))
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

pub fn get_selected_text_with_context() -> Result<(String, Option<String>), Box<dyn std::error::Error>> {
    debug_println!("[CONTEXT_MACOS] Attempting to get selected text and AX description context.");
    // Directly call the enhanced AX function which now returns (String, Option<String>)
    match get_selected_text_by_ax() {
        Ok((selected_text, context_option)) => {
            if selected_text.is_empty() && context_option.is_none() {
                 // If both are empty, it might indicate an issue or no actual selection/context
                 debug_println!("[CONTEXT_MACOS] Both selected text and AX context are empty.");
                 // Depending on desired behavior, could return an error or Ok with empty values
                 // For now, let's return Ok as per previous logic that allowed empty selections.
            }
            debug_println!("[CONTEXT_MACOS] Selected text: '{}', AX Context: '{:?}'", selected_text, context_option);
            Ok((selected_text, context_option))
        }
        Err(e) => {
            debug_println!("[CONTEXT_MACOS] Error in get_selected_text_by_ax: {:?}", e);
            Err(e)
        }
    }
}
