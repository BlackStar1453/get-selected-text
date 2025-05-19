use debug_print::debug_println;
use enigo::*;
use parking_lot::Mutex;
use std::{thread, time::Duration};

use crate::GetTextError;

static COPY_PASTE_LOCKER: Mutex<()> = Mutex::new(());
static INPUT_LOCK_LOCKER: Mutex<()> = Mutex::new(());

const CONTEXT_CHARS_BEFORE: usize = 150;
const CONTEXT_CHARS_AFTER: usize = 150;
const CLIPBOARD_OPERATION_TIMEOUT_MS: u64 = 5000; // 5秒超时

// Use debug_print for logging if enabled, otherwise println
#[cfg(debug_assertions)]
use debug_print::debug_println as log_println;
#[cfg(not(debug_assertions))]
use println as log_println;

pub(crate) fn right_arrow_click(enigo: &mut Enigo, n: usize) {
    let _guard = INPUT_LOCK_LOCKER.lock();

    for _ in 0..n {
        enigo.key(Key::RightArrow, Direction::Click).unwrap();
    }
}

pub(crate) fn left_arrow_click(enigo: &mut Enigo, n: usize) {
    let _guard = INPUT_LOCK_LOCKER.lock();

    for _ in 0..n {
        enigo.key(Key::LeftArrow, Direction::Click).unwrap();
    }
}

pub(crate) fn up_control_keys(enigo: &mut Enigo) {
    enigo.key(Key::Control, Direction::Release).unwrap();
    enigo.key(Key::Alt, Direction::Release).unwrap();
    enigo.key(Key::Shift, Direction::Release).unwrap();
    enigo.key(Key::Space, Direction::Release).unwrap();
    enigo.key(Key::Tab, Direction::Release).unwrap();
    #[cfg(target_os = "macos")]
    enigo.key(Key::Meta, Direction::Release).unwrap();
}

pub(crate) fn copy(enigo: &mut Enigo) {

    log_println!("[COPY] Calling up_control_keys...");
    crate::utils::up_control_keys(enigo);
    log_println!("[COPY] up_control_keys finished.");

    log_println!("[COPY] Simulating Control Press...");
    enigo.key(Key::Control, Direction::Press).unwrap();
    log_println!("[COPY] Control Press finished.");

    #[cfg(target_os = "windows")]
    {
        log_println!("[COPY] Simulating C Click...");
        enigo.key(Key::C, Direction::Click).unwrap();
        log_println!("[COPY] C Click finished.");
    }
    #[cfg(target_os = "linux")]
    {
        log_println!("[COPY] Simulating Unicode 'c' Click...");
        enigo.key(Key::Unicode('c'), Direction::Click).unwrap();
        log_println!("[COPY] Unicode 'c' Click finished.");
    }
    // No macOS specific key needed here as per original code in utils.rs

    log_println!("[COPY] Simulating Control Release...");
    enigo.key(Key::Control, Direction::Release).unwrap();
    log_println!("[COPY] Control Release finished.");

    log_println!("[COPY] Releasing COPY_PASTE_LOCKER...");
    // _guard goes out of scope here, lock released automatically
} 

pub(crate) fn get_selected_text_by_clipboard(
    enigo: &mut Enigo,
    cancel_select: bool,
) -> Result<String, Box<dyn std::error::Error>> {
    use arboard::Clipboard;

    let old_clipboard = (Clipboard::new()?.get_text(), Clipboard::new()?.get_image());

    let mut write_clipboard = Clipboard::new()?;

    let not_selected_placeholder = "";

    write_clipboard.set_text(not_selected_placeholder)?;

    thread::sleep(Duration::from_millis(50));

    copy(enigo);

    if cancel_select {
        crate::utils::right_arrow_click(enigo, 1);
    }

    thread::sleep(Duration::from_millis(250));

    let new_text = Clipboard::new()?.get_text();

    match old_clipboard {
        (Ok(old_text), _) => {
            // Old Content is Text
            write_clipboard.set_text(old_text.clone())?;
            if let Ok(new) = new_text {
                if new.trim() == not_selected_placeholder.trim() {
                    Ok(String::new())
                } else {
                    Ok(new)
                }
            } else {
                Ok(String::new())
            }
        }
        (_, Ok(image)) => {
            // Old Content is Image
            write_clipboard.set_image(image)?;
            if let Ok(new) = new_text {
                if new.trim() == not_selected_placeholder.trim() {
                    Ok(String::new())
                } else {
                    Ok(new)
                }
            } else {
                Ok(String::new())
            }
        }
        _ => {
            // Old Content is Empty
            write_clipboard.clear()?;
            if let Ok(new) = new_text {
                if new.trim() == not_selected_placeholder.trim() {
                    Ok(String::new())
                } else {
                    Ok(new)
                }
            } else {
                Ok(String::new())
            }
        }
    }
}

pub(crate) fn get_context_via_select_all(
    enigo: &mut Enigo,
    selected_text: &str,
) -> Result<Option<String>, Box<dyn std::error::Error>> {
    use arboard::Clipboard;
    use std::time::{Duration, Instant};
    
    log_println!("[SELECT_ALL] Starting get_context_via_select_all...");
    
    if selected_text.is_empty() {
        // Cannot find context if the original selection was empty
        log_println!("[SELECT_ALL] Selected text is empty, returning None.");
        return Ok(None);
    }

    let _guard = COPY_PASTE_LOCKER.lock();
    log_println!("[SELECT_ALL] Acquired COPY_PASTE_LOCKER.");

    // 添加总体超时
    let start_time = Instant::now();
    
    // --- Save original clipboard ---  
    log_println!("[SELECT_ALL] Getting original clipboard content...");
    let mut clipboard = Clipboard::new().map_err(|e| GetTextError::Other(e.to_string()))?;
    let old_clipboard_text = clipboard.get_text();
    let old_clipboard_image = clipboard.get_image(); 
    log_println!("[SELECT_ALL] Original clipboard content retrieved.");

    // --- Perform Select All + Copy --- 
    log_println!("[SELECT_ALL] Releasing modifier keys...");
    crate::utils::up_control_keys(enigo); // Release modifier keys
    
    thread::sleep(Duration::from_millis(50)); 
    
    if start_time.elapsed().as_millis() > CLIPBOARD_OPERATION_TIMEOUT_MS as u128 {
        log_println!("[SELECT_ALL] Timeout before Select All. Abort.");
        return Err(Box::new(GetTextError::Other("Operation timed out".to_string())));
    }

    // Simulate Ctrl+A (or Cmd+A on macOS)
    log_println!("[SELECT_ALL] Simulating Select All...");
    #[cfg(target_os = "macos")]
    enigo.key(Key::Meta, Direction::Press).unwrap();
    #[cfg(not(target_os = "macos"))]
    enigo.key(Key::Command, Direction::Press).unwrap();

    #[cfg(target_os = "windows")]
    enigo.key(Key::A, Direction::Click).unwrap();
    #[cfg(target_os = "linux")]
    enigo.key(Key::Unicode('a'), Direction::Click).unwrap();
    #[cfg(target_os = "macos")]
    enigo.key(Key::Unicode('a'), Direction::Click).unwrap();

    #[cfg(target_os = "macos")]
    enigo.key(Key::Meta, Direction::Release).unwrap();
    #[cfg(not(target_os = "macos"))]
    enigo.key(Key::Control, Direction::Release).unwrap();
    
    thread::sleep(Duration::from_millis(50)); 
    
    if start_time.elapsed().as_millis() > CLIPBOARD_OPERATION_TIMEOUT_MS as u128 {
        log_println!("[SELECT_ALL] Timeout before Copy. Abort.");
        return Err(Box::new(GetTextError::Other("Operation timed out".to_string())));
    }

    log_println!("[SELECT_ALL] Simulating Copy...");
    copy(enigo); // Simulate Ctrl+C (or Cmd+C)

    log_println!("[SELECT_ALL] Copy simulation finished.");

    thread::sleep(Duration::from_millis(100)); // Wait for clipboard update

    // --- 取消全文选中状态 ---
    log_println!("[SELECT_ALL] 尝试取消全文选中状态...");
    
    // 方法1: 先尝试ESC键，这在许多应用中都可以取消选择
    thread::sleep(Duration::from_millis(50));
    log_println!("[SELECT_ALL] 方法1：尝试使用ESC键取消选择");
    enigo.key(Key::Escape, Direction::Click).unwrap();
    thread::sleep(Duration::from_millis(100));
    
    // 方法2: 尝试按左箭头键
    log_println!("[SELECT_ALL] 方法2：尝试使用左箭头键取消选择");
    crate::utils::left_arrow_click(enigo, 1);
    thread::sleep(Duration::from_millis(100));
    
    // 方法3: 尝试按右箭头键
    log_println!("[SELECT_ALL] 方法3：尝试使用右箭头键取消选择");
    crate::utils::right_arrow_click(enigo, 1);
    thread::sleep(Duration::from_millis(100));
    
    // 方法4: 尝试单击以取消选择（这在某些应用中有效）
    log_println!("[SELECT_ALL] 方法4：尝试使用单击操作取消选择");
    enigo.key(Key::Control, Direction::Release).unwrap(); // 确保没有修饰键被按下
    enigo.key(Key::Shift, Direction::Release).unwrap();
    enigo.key(Key::Alt, Direction::Release).unwrap();
    thread::sleep(Duration::from_millis(50));
    // 注意：实际点击操作可能需要鼠标位置信息，这里只是确保释放了所有修饰键
    
    log_println!("[SELECT_ALL] 完成尝试取消全文选中");

    log_println!("[SELECT_ALL] Sleep finished, attempting to get clipboard content...");
    

    if start_time.elapsed().as_millis() > CLIPBOARD_OPERATION_TIMEOUT_MS as u128 {
        log_println!("[SELECT_ALL] Timeout before getting clipboard content. Abort.");
        return Err(Box::new(GetTextError::Other("Operation timed out".to_string())));
    }

    // --- Get Full Text ---  
    log_println!("[SELECT_ALL] Getting clipboard content after Select All + Copy...");
    let full_text_result = Clipboard::new()
        .map_err(|e| GetTextError::Other(e.to_string()))?
        .get_text();
    log_println!("[SELECT_ALL] Clipboard content retrieved: {}", full_text_result.is_ok());

    // --- Restore original clipboard (important!) ---
    log_println!("[SELECT_ALL] Restoring original clipboard...");
    match (old_clipboard_text, old_clipboard_image) {
        (Ok(text), _) => clipboard
            .set_text(text)
            .map_err(|e| GetTextError::Other(e.to_string()))?,
        (_, Ok(image)) => clipboard
            .set_image(image)
            .map_err(|e| GetTextError::Other(e.to_string()))?,
        _ => clipboard
            .clear()
            .map_err(|e| GetTextError::Other(e.to_string()))?,
    }
    log_println!("[SELECT_ALL] Original clipboard restored.");
    
    // --- Process Full Text ---  
    match full_text_result {
        Ok(full_text) => {
            log_println!("[SELECT_ALL] Processing full text ({} chars)...", full_text.len());
            if let Some(start_pos) = full_text.find(selected_text) {
                log_println!("[SELECT_ALL] Selected text found at position {}", start_pos);
                let end_pos = start_pos + selected_text.len();
                let context_start = start_pos.saturating_sub(CONTEXT_CHARS_BEFORE);
                let context_end = (end_pos + CONTEXT_CHARS_AFTER).min(full_text.len());
                
                log_println!("[SELECT_ALL] Extracting context from {} to {}", context_start, context_end);
                // Ensure we are extracting valid UTF-8 boundaries
                let mut valid_start = context_start;
                while !full_text.is_char_boundary(valid_start) && valid_start < full_text.len() {
                    valid_start += 1;
                }
                
                let mut valid_end = context_end;
                while !full_text.is_char_boundary(valid_end) && valid_end > valid_start {
                    valid_end -= 1;
                }

                if valid_start < valid_end {
                    let context = full_text[valid_start..valid_end].to_string();
                    log_println!("[SELECT_ALL] Context extracted successfully ({} chars).", context.len());
                    Ok(Some(context))
                } else {
                    log_println!("[SELECT_ALL] Invalid context boundaries. Returning full text.");
                     Ok(Some(full_text)) // Fallback to full text if boundaries are weird
                }
            } else {
                // Selected text not found in the full text copied via Ctrl+A
                log_println!("[SELECT_ALL] Selected text not found in full text.");
                Err(Box::new(GetTextError::NotInContext))
            }
        }
        Err(e) => {
            // Failed to get text after Select All + Copy
            log_println!("[SELECT_ALL] Failed to get text from clipboard: {}", e);
            Err(Box::new(GetTextError::Other("Failed to get text after Select All".to_string())))
        }
    }
}
