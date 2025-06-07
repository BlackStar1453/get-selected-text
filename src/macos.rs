use std::num::NonZeroUsize;

use accessibility_ng::{AXAttribute, AXUIElement, AXUIElementAttributes, AXValue};
use accessibility_sys_ng::{kAXFocusedUIElementAttribute, kAXSelectedTextAttribute};
use active_win_pos_rs::get_active_window;
use core_foundation::string::CFString;
use core_foundation::base::{TCFType, CFType};
use core_foundation::number::CFNumber;
use core_foundation::boolean::CFBoolean;
use core_foundation::attributed_string::CFAttributedString;
use core_foundation::array::CFArray;
use debug_print::debug_println;
use lru::LruCache;
use parking_lot::Mutex;
use enigo::{Enigo, Mouse, Settings};

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
            return get_selected_text_by_ax_robust().map(|(text, _context)| text);
        }
        return get_selected_text_by_clipboard_using_applescript();
    }

    match get_selected_text_by_ax_robust() {
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

// 新的健壮版本的 AX 获取方法
fn get_selected_text_by_ax_robust() -> Result<(String, Option<String>), Box<dyn std::error::Error>> {
    debug_println!("[AX_ROBUST] Starting robust AX text retrieval...");
    
    // 策略1: 尝试获取系统级别的 focused element
    debug_println!("[AX_ROBUST] Strategy 1: Attempting system-wide focused element...");
    if let Ok(result) = try_system_focused_element() {
        debug_println!("[AX_ROBUST] Strategy 1 succeeded!");
        return Ok(result);
    }
    
    // 策略2: 通过活动窗口获取
    debug_println!("[AX_ROBUST] Strategy 2: Attempting active window approach...");
    if let Ok(result) = try_active_window_approach() {
        debug_println!("[AX_ROBUST] Strategy 2 succeeded!");
        return Ok(result);
    }
    
    // 策略3: 尝试使用替代的 AX 属性和方法
    debug_println!("[AX_ROBUST] Strategy 3: Attempting alternative AX attributes...");
    if let Ok(result) = try_alternative_ax_methods() {
        debug_println!("[AX_ROBUST] Strategy 3 succeeded!");
        return Ok(result);
    }
    
    debug_println!("[AX_ROBUST] All strategies failed");
    Err(Box::new(std::io::Error::new(
        std::io::ErrorKind::NotFound,
        "All AX strategies failed to find UI element with selected text",
    )))
}

// 策略1: 原始的系统级别方法
fn try_system_focused_element() -> Result<(String, Option<String>), Box<dyn std::error::Error>> {
    debug_println!("[AX_STRATEGY1] Trying system-wide focused element...");
    let system_element = AXUIElement::system_wide();
    
    let focused_element = match system_element
        .attribute(&AXAttribute::new(&CFString::from_static_string(
            kAXFocusedUIElementAttribute,
        )))
        .ok()
        .and_then(|element| element.downcast_into::<AXUIElement>())
    {
        Some(element) => element,
        None => {
            debug_println!("[AX_STRATEGY1] No system-wide focused UI element found.");
            return Err(Box::new(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "No system-wide focused UI element",
            )));
        }
    };

    extract_text_and_context(&focused_element)
}

// 策略2: 通过活动窗口获取
fn try_active_window_approach() -> Result<(String, Option<String>), Box<dyn std::error::Error>> {
    debug_println!("[AX_STRATEGY2] Trying active window approach...");
    
    let active_window = get_active_window()
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::NotFound, format!("Failed to get active window: {:?}", e)))?;
    
    debug_println!("[AX_STRATEGY2] Active window: {} (PID: {:?})", active_window.app_name, active_window.process_id);
    
    // 通过进程ID获取应用程序的AX元素
    let app_element = AXUIElement::application(active_window.process_id as i32);
    
    // 首先记录应用程序元素的所有属性
    debug_println!("[AX_STRATEGY2] === 应用程序元素属性 ===");
    log_element_attributes(&app_element, "App");
    
    // 尝试获取应用的focused element
    if let Some(focused_element) = app_element
        .attribute(&AXAttribute::new(&CFString::from_static_string(
            kAXFocusedUIElementAttribute,
        )))
        .ok()
        .and_then(|element| element.downcast_into::<AXUIElement>())
    {
        debug_println!("[AX_STRATEGY2] Found focused element via application");
        debug_println!("[AX_STRATEGY2] === Focused元素属性 ===");
        log_element_attributes(&focused_element, "Focused");
        
        if let Ok(result) = extract_text_and_context(&focused_element) {
            return Ok(result);
        }
    }
    
    debug_println!("[AX_STRATEGY2] No focused element found via application, starting deep traversal...");
    
    // 开始深度遍历寻找包含选中文本的元素
    if let Some(result) = traverse_ui_tree(&app_element, 0, "App") {
        debug_println!("[AX_STRATEGY2] Found result via deep traversal");
        return Ok(result);
    }
    
    debug_println!("[AX_STRATEGY2] Deep traversal also failed");
    Err(Box::new(std::io::Error::new(
        std::io::ErrorKind::NotFound,
        "No focused element found via active window approach",
    )))
}

// 深度遍历UI元素树
fn traverse_ui_tree(element: &AXUIElement, depth: usize, element_name: &str) -> Option<(String, Option<String>)> {
    const MAX_DEPTH: usize = 6;
    const MAX_CHILDREN_PER_LEVEL: usize = 15;
    
    if depth > MAX_DEPTH {
        debug_println!("[UI_TRAVERSE] Reached max depth {}, stopping", depth);
        return None;
    }
    
    let indent = "  ".repeat(depth);
    debug_println!("[UI_TRAVERSE] {}Depth {}: Checking {} element", indent, depth, element_name);
    
    // 记录当前元素的属性
    log_element_attributes(element, &format!("{}Depth{}", indent, depth));
    
    // 检查当前元素是否有选中文本
    if let Ok((selected_text, context)) = extract_text_and_context(element) {
        if !selected_text.is_empty() {
            debug_println!("[UI_TRAVERSE] {}✓ Found selected text: '{}'", indent, selected_text);
            return Some((selected_text, context));
        }
    }
    
    // 尝试获取子元素
    debug_println!("[UI_TRAVERSE] {}Getting children...", indent);
    if let Ok(children_attr) = element.attribute(&AXAttribute::new(&CFString::from_static_string("AXChildren"))) {
        debug_println!("[UI_TRAVERSE] {}Found AXChildren attribute", indent);
        
        // 尝试使用更安全的方式获取子元素
        if let Some(children_count) = get_children_count(element) {
            debug_println!("[UI_TRAVERSE] {}Found {} children", indent, children_count);
            
            let search_limit = children_count.min(MAX_CHILDREN_PER_LEVEL);
            
            for i in 0..search_limit {
                if let Some(child) = get_child_at_index(element, i) {
                    debug_println!("[UI_TRAVERSE] {}Checking child {}/{}", indent, i + 1, search_limit);
                    
                    let child_name = get_element_role(&child).unwrap_or_else(|| format!("Child{}", i));
                    
                    if let Some(result) = traverse_ui_tree(&child, depth + 1, &child_name) {
                        return Some(result);
                    }
                } else {
                    debug_println!("[UI_TRAVERSE] {}Failed to get child at index {}", indent, i);
                }
            }
            
            if children_count > MAX_CHILDREN_PER_LEVEL {
                debug_println!("[UI_TRAVERSE] {}Limited search to {} children (total: {})", 
                              indent, MAX_CHILDREN_PER_LEVEL, children_count);
            }
        } else {
            debug_println!("[UI_TRAVERSE] {}Could not determine children count", indent);
        }
    } else {
        debug_println!("[UI_TRAVERSE] {}No AXChildren attribute found", indent);
    }
    
    None
}

// 记录元素的所有重要属性
fn log_element_attributes(element: &AXUIElement, prefix: &str) {
    let attributes_to_check = [
        ("AXRole", "角色"),
        ("AXSubrole", "子角色"),
        ("AXRoleDescription", "角色描述"),
        ("AXTitle", "标题"),
        ("AXDescription", "描述"),
        ("AXValue", "值"),
        ("AXSelectedText", "选中文本"),
        ("AXHelp", "帮助"),
        ("AXPlaceholderValue", "占位符"),
        ("AXIdentifier", "标识符"),
        ("AXEnabled", "是否启用"),
        ("AXFocused", "是否聚焦"),
        ("AXSelected", "是否选中"),
        ("AXNumberOfCharacters", "字符数"),
        ("AXSelectedTextRange", "选中文本范围"),
        ("AXVisibleCharacterRange", "可见字符范围"),
    ];
    
    for (attr_name, description) in &attributes_to_check {
        if let Ok(attr_value) = element.attribute(&AXAttribute::new(&CFString::from_static_string(attr_name))) {
            // 尝试不同的类型转换，避免移动所有权
            if let Some(string_val) = attr_value.clone().downcast_into::<CFString>() {
                let text = string_val.to_string();
                if !text.is_empty() {
                    debug_println!("[{}] {}: '{}' = '{}'", prefix, attr_name, description, text);
                }
            } else if let Some(number_val) = attr_value.clone().downcast_into::<CFNumber>() {
                if let Some(num) = number_val.to_i64() {
                    debug_println!("[{}] {}: '{}' = {}", prefix, attr_name, description, num);
                }
            } else if let Some(_bool_val) = attr_value.clone().downcast_into::<CFBoolean>() {
                debug_println!("[{}] {}: '{}' = <布尔值>", prefix, attr_name, description);
            } else {
                debug_println!("[{}] {}: '{}' = <复杂类型>", prefix, attr_name, description);
            }
        }
    }
}

// 获取子元素数量
fn get_children_count(element: &AXUIElement) -> Option<usize> {
    if let Ok(children_attr) = element.attribute(&AXAttribute::new(&CFString::from_static_string("AXChildren"))) {
        if let Some(children_array) = children_attr.downcast_into::<CFArray>() {
            let len = children_array.len();
            if len >= 0 {
                return Some(len as usize);
            }
        }
    }
    None
}

// 获取指定索引的子元素
fn get_child_at_index(element: &AXUIElement, index: usize) -> Option<AXUIElement> {
    if let Ok(children_attr) = element.attribute(&AXAttribute::new(&CFString::from_static_string("AXChildren"))) {
        if let Some(children_array) = children_attr.downcast_into::<CFArray>() {
            let len = children_array.len();
            if len > 0 && (index as isize) < len {
                if let Some(child_ref) = children_array.get(index as isize) {
                    // 使用 CFType 作为通用的包装器来解决类型推断问题
                    // 解引用 ItemRef 以获取裸指针 *const c_void
                    let cf_type = unsafe { CFType::wrap_under_get_rule(*child_ref) };

                    if let Some(ax_element) = cf_type.downcast_into::<AXUIElement>() {
                        debug_println!("[CHILD_ACCESS] Successfully converted child at index {} to AXUIElement", index);
                        return Some(ax_element);
                    } else {
                        debug_println!("[CHILD_ACCESS] Failed to downcast CFType to AXUIElement at index {}", index);
                    }
                }
            }
        }
    }
    None
}

// 获取元素的角色信息
fn get_element_role(element: &AXUIElement) -> Option<String> {
    if let Ok(role_attr) = element.attribute(&AXAttribute::new(&CFString::from_static_string("AXRole"))) {
        if let Some(role_string) = role_attr.downcast_into::<CFString>() {
            return Some(role_string.to_string());
        }
    }
    None
}

// 策略3: 尝试使用替代的 AX 属性和方法
fn try_alternative_ax_methods() -> Result<(String, Option<String>), Box<dyn std::error::Error>> {
    debug_println!("[AX_STRATEGY3] Trying alternative AX methods as a last resort...");
    
    let active_window = get_active_window()
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::NotFound, format!("Failed to get active window: {:?}", e)))?;
    
    let app_element = AXUIElement::application(active_window.process_id as i32);
    
    // 方法1: 尝试直接从应用元素获取 AXSelectedText
    debug_println!("[AX_STRATEGY3] Trying AXSelectedText directly on application element");
    if let Ok(attr_value) = app_element.attribute(&AXAttribute::new(&CFString::from_static_string("AXSelectedText"))) {
        if let Some(text_value) = attr_value.downcast_into::<CFString>() {
            let text = text_value.to_string();
            if !text.is_empty() {
                debug_println!("[AX_STRATEGY3] Found text via AXSelectedText: '{}'", text);
                return Ok((text, None));
            }
        }
    }

    // 方法2: 尝试检查剪贴板是否包含最近复制的文本
    debug_println!("[AX_STRATEGY3] Trying clipboard inspection...");
    if let Ok(clipboard_text) = get_current_clipboard_text() {
        if !clipboard_text.is_empty() && clipboard_text.len() < 1000 { // 放宽长度限制
            debug_println!("[AX_STRATEGY3] Found potential selected text from clipboard: '{}'", clipboard_text);
            return Ok((clipboard_text, None));
        }
    }
    
    debug_println!("[AX_STRATEGY3] All alternative methods failed");
    Err(Box::new(std::io::Error::new(
        std::io::ErrorKind::NotFound,
        "No selected text found via alternative AX methods",
    )))
}

// 获取当前剪贴板文本的辅助函数
fn get_current_clipboard_text() -> Result<String, Box<dyn std::error::Error>> {
    use std::process::Command;
    
    let output = Command::new("pbpaste").output()?;
    if output.status.success() {
        let text = String::from_utf8(output.stdout)?;
        Ok(text)
    } else {
        Err("Failed to get clipboard content".into())
    }
}

// 从UI元素提取文本和上下文的通用方法
fn extract_text_and_context(element: &AXUIElement) -> Result<(String, Option<String>), Box<dyn std::error::Error>> {
    debug_println!("[AX_EXTRACT] Extracting text and context from element");
    
    // 首先尝试获取选中文本
    let selected_text = match element.attribute(&AXAttribute::new(&CFString::from_static_string(kAXSelectedTextAttribute))) {
        Ok(selected_text_cfvalue) => {
            if let Some(selected_text_cfstring) = selected_text_cfvalue.downcast_into::<CFString>() {
                let text = selected_text_cfstring.to_string();
                debug_println!("[AX_EXTRACT] Found selected text: '{}'", text);
                text
            } else {
                debug_println!("[AX_EXTRACT] Selected text attribute was not a CFString");
                String::new()
            }
        }
        Err(e) => {
            debug_println!("[AX_EXTRACT] Failed to get selected text: {:?}", e);
            String::new()
        }
    };
    
    // 如果没有选中文本，返回错误
    if selected_text.is_empty() {
        return Err(Box::new(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "No selected text found in element",
        )));
    }
    
    // 尝试获取上下文
    let context = get_context_from_element(element);
    
    // 针对 WebArea 的特殊处理：如果找到了选中文本但没有上下文，则强制触发 fallback
    if get_element_role(element).as_deref() == Some("AXWebArea") && context.is_none() {
        debug_println!("[AX_EXTRACT] Found selected text in WebArea but no AXValue context. Forcing an error to trigger AppleScript fallback.");
        return Err(Box::new(std::io::Error::new(
            std::io::ErrorKind::Other,
            "Found text in WebArea but context requires fallback",
        )));
    }

    Ok((selected_text, context))
}

// 获取上下文的方法
fn get_context_from_element(element: &AXUIElement) -> Option<String> {
    debug_println!("[AX_CONTEXT] Attempting to get context from element");
    let role = get_element_role(element);

    // Special handling for WebArea: only trust AXValue
    if role.as_deref() == Some("AXWebArea") {
        debug_println!("[AX_CONTEXT] Element is a WebArea. Prioritizing AXValue for context.");
        if let Ok(cf_type_val) = element.value() {
            if let Some(s) = cf_type_val.downcast_into::<CFString>() {
                let text = s.to_string();
                if !text.is_empty() {
                    debug_println!("[AX_CONTEXT] Found context for WebArea from AXValue (length: {})", text.len());
                    return Some(text);
                }
            }
        }
        debug_println!("[AX_CONTEXT] Could not get context from AXValue for WebArea. Returning None to avoid using incorrect fallbacks like title.");
        return None; // For WebArea, do NOT fall back to title or description
    }
    
    // Fallback logic for other element types
    // 策略1: 从 AXValue 获取
    if let Ok(cf_type_val) = element.value() {
        if let Some(s) = cf_type_val.downcast_into::<CFString>() {
            let text = s.to_string();
            if !text.is_empty() && text.len() > 10 { // 确保有足够的内容作为上下文
                debug_println!("[AX_CONTEXT] Found context from AXValue (length: {})", text.len());
                return Some(text);
            }
        }
    }
    
    // 策略2: 从描述获取
    if let Ok(cf_string) = element.description() {
        let desc_text = cf_string.to_string();
        if !desc_text.is_empty() && desc_text.len() > 10 {
            debug_println!("[AX_CONTEXT] Found context from description (length: {})", desc_text.len());
            return Some(desc_text);
        }
    }
    
    // 策略3: 从标题获取
    if let Ok(cf_string) = element.title() {
        let title_text = cf_string.to_string();
        if !title_text.is_empty() && title_text.len() > 10 {
            debug_println!("[AX_CONTEXT] Found context from title (length: {})", title_text.len());
            return Some(title_text);
        }
    }
    
    debug_println!("[AX_CONTEXT] No context found from element attributes");
    None
}

// 保持原有的 get_selected_text_by_ax 函数以兼容性
fn get_selected_text_by_ax() -> Result<(String, Option<String>), Box<dyn std::error::Error>> {
    // 直接调用新的健壮版本
    get_selected_text_by_ax_robust()
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
    match get_selected_text_by_ax_robust() {
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
            debug_println!("[CONTEXT_MACOS] Error in get_selected_text_by_ax_robust: {:?}. Falling back to AppleScript.", e);
            // 改进的fallback：尝试使用AppleScript获取上下文
            get_selected_text_with_context_applescript()
        }
    }
}

// 新增：使用AppleScript获取选中文本和上下文的方法
fn get_selected_text_with_context_applescript() -> Result<(String, Option<String>), Box<dyn std::error::Error>> {
    debug_println!("[APPLESCRIPT_CONTEXT] Attempting to get selected text and context via AppleScript");
    
    // 首先获取选中文本
    let selected_text = get_selected_text_by_clipboard_using_applescript()?;
    
    if selected_text.is_empty() {
        return Ok((selected_text, None));
    }
    
    // 尝试获取上下文
    match get_context_via_applescript() {
        Ok(context) => {
            if context.contains(&selected_text) {
                debug_println!("[APPLESCRIPT_CONTEXT] Found context containing selected text");
                Ok((selected_text, Some(context)))
            } else {
                debug_println!("[APPLESCRIPT_CONTEXT] Context doesn't contain selected text, returning without context");
                Ok((selected_text, None))
            }
        }
        Err(e) => {
            debug_println!("[APPLESCRIPT_CONTEXT] Failed to get context: {:?}", e);
            Ok((selected_text, None))
        }
    }
}

// AppleScript脚本获取上下文
fn get_context_via_applescript() -> Result<String, Box<dyn std::error::Error>> {
    const CONTEXT_SCRIPT: &str = r#"
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

-- 尝试获取更多上下文：模拟 Cmd+A 来选择全部文本
tell application "System Events" to keystroke "a" using {command down}
delay 0.05

-- Copy all text to clipboard:
tell application "System Events" to keystroke "c" using {command down}
delay 0.1

-- 恢复警告音量
tell application "System Events"
    set volume alert volume savedAlertVolume
end tell

-- 检查剪贴板是否有变化
if thePasteboard's changeCount() is theCount then
    set the clipboard to savedClipboard
    return ""
end if

set theFullText to the clipboard

-- 恢复原始剪贴板内容
set the clipboard to savedClipboard

-- 按ESC键取消全选状态
tell application "System Events" to keystroke (ASCII character 27)
delay 0.05

-- 再按一下左箭头确保取消选择
tell application "System Events" to key code 123

theFullText
"#;

    debug_println!("[APPLESCRIPT_CONTEXT] Executing context retrieval script");
    let output = std::process::Command::new("osascript")
        .arg("-e")
        .arg(CONTEXT_SCRIPT)
        .output()?;
    
    if output.status.success() {
        let content = String::from_utf8(output.stdout)?;
        let content = content.trim();
        debug_println!("[APPLESCRIPT_CONTEXT] Retrieved context length: {}", content.len());
        Ok(content.to_string())
    } else {
        let err = String::from_utf8(output.stderr)?;
        debug_println!("[APPLESCRIPT_CONTEXT] Script failed: {}", err);
        Err(err.into())
    }
}
