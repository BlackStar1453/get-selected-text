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

pub(crate) fn up_control_keys(enigo: &mut Enigo) {
    enigo.key(Key::Control, Direction::Release).unwrap();
    enigo.key(Key::Alt, Direction::Release).unwrap();
    enigo.key(Key::Shift, Direction::Release).unwrap();
    enigo.key(Key::Space, Direction::Release).unwrap();
    enigo.key(Key::Tab, Direction::Release).unwrap();
}

pub(crate) fn copy(enigo: &mut Enigo) {
    let _guard = COPY_PASTE_LOCKER.lock();

    crate::utils::up_control_keys(enigo);

    enigo.key(Key::Control, Direction::Press).unwrap();
    #[cfg(target_os = "windows")]
    enigo.key(Key::C, Direction::Click).unwrap();
    #[cfg(target_os = "linux")]
    enigo.key(Key::Unicode('c'), Direction::Click).unwrap();
    enigo.key(Key::Control, Direction::Release).unwrap();
}

// 新增一个带超时的版本，用于内部调用
fn get_selected_text_by_clipboard_internal(
    enigo: &mut Enigo,
    cancel_select: bool,
) -> Result<String, GetTextError> {
    use arboard::Clipboard;
    use std::sync::{Arc, Mutex};
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::thread;
    use std::time::{Duration, Instant};

    log_println!("[CLIPBOARD] Starting clipboard operation...");
    let _guard = COPY_PASTE_LOCKER.lock();
    log_println!("[CLIPBOARD] Acquired COPY_PASTE_LOCKER.");

    // 创建一个可以在线程间共享的结果
    let result = Arc::new(Mutex::new(None));
    let timeout_flag = Arc::new(AtomicBool::new(false));
    
    // 克隆Arc以便在线程中使用
    let thread_result = Arc::clone(&result);
    let thread_timeout = Arc::clone(&timeout_flag);
    
    // 启动一个线程执行实际的剪贴板操作
    let clipboard_thread = thread::spawn(move || {
        let clipboard_thread_start = Instant::now();
        log_println!("[CLIPBOARD] Thread started at {:?}", clipboard_thread_start);
        
        let clipboard_result = (|| -> Result<String, GetTextError> {
            // 在线程中执行剪贴板操作
            log_println!("[CLIPBOARD] Creating clipboard instance...");
            let clipboard_instance_result = Clipboard::new();
            if let Err(e) = &clipboard_instance_result {
                log_println!("[CLIPBOARD] Failed to create clipboard: {}", e);
                return Err(GetTextError::Other(e.to_string()));
            }
            let mut clipboard = clipboard_instance_result.unwrap();
            
            log_println!("[CLIPBOARD] Getting old clipboard content...");
            let old_clipboard_text = clipboard.get_text();
            let old_clipboard_image = clipboard.get_image();
            log_println!("[CLIPBOARD] Old clipboard text retrieved: {}", old_clipboard_text.is_ok());
            log_println!("[CLIPBOARD] Old clipboard image retrieved: {}", old_clipboard_image.is_ok());
            
            let not_selected_placeholder = "";
            
            log_println!("[CLIPBOARD] Setting placeholder text...");
            if let Err(e) = clipboard.set_text(not_selected_placeholder) {
                log_println!("[CLIPBOARD] Failed to set placeholder: {}", e);
                return Err(GetTextError::Other(e.to_string()));
            }
            
            thread::sleep(Duration::from_millis(50));
            
            // 检查是否超时
            if thread_timeout.load(Ordering::SeqCst) {
                log_println!("[CLIPBOARD] Operation timed out before copy.");
                return Err(GetTextError::Other("Clipboard operation timed out".to_string()));
            }
            
            log_println!("[CLIPBOARD] Simulating copy operation...");
            // 这里不传入enigo，因为我们在另一个线程中，不能共享可变引用
            // 我们会在主线程中执行copy操作
            
            log_println!("[CLIPBOARD] Waiting for clipboard update...");
            thread::sleep(Duration::from_millis(1000)); // 增加等待时间到 1000ms
            
            log_println!("[CLIPBOARD] Getting new clipboard content...");
            let new_clipboard = Clipboard::new();
            if let Err(e) = &new_clipboard {
                log_println!("[CLIPBOARD] Failed to create new clipboard: {}", e);
                return Err(GetTextError::Other(e.to_string()));
            }
            let new_text_result = new_clipboard.unwrap().get_text();
            log_println!("[CLIPBOARD] New text retrieved: {}", new_text_result.is_ok());
            
            // 恢复剪贴板
            log_println!("[CLIPBOARD] Restoring original clipboard...");
            match (old_clipboard_text, old_clipboard_image) {
                (Ok(text), _) => {
                    if let Err(e) = clipboard.set_text(text) {
                        log_println!("[CLIPBOARD] Failed to restore text: {}", e);
                        return Err(GetTextError::Other(e.to_string()));
                    }
                },
                (_, Ok(image)) => {
                    if let Err(e) = clipboard.set_image(image) {
                        log_println!("[CLIPBOARD] Failed to restore image: {}", e);
                        return Err(GetTextError::Other(e.to_string()));
                    }
                },
                _ => {
                    if let Err(e) = clipboard.clear() {
                        log_println!("[CLIPBOARD] Failed to clear clipboard: {}", e);
                        return Err(GetTextError::Other(e.to_string()));
                    }
                },
            }
            
            // 处理结果
            match new_text_result {
                Ok(new_text) => {
                    if new_text.trim() == not_selected_placeholder.trim() {
                        log_println!("[CLIPBOARD] No text was selected (got placeholder).");
                        Ok(String::new())
                    } else {
                        log_println!("[CLIPBOARD] Successfully got selected text ({} chars).", new_text.len());
                        Ok(new_text)
                    }
                },
                Err(e) => {
                    log_println!("[CLIPBOARD] Failed to get new text: {}", e);
                    Err(GetTextError::Other(e.to_string()))
                }
            }
        })();
        
        // 保存结果到共享变量
        if let Ok(mut result_guard) = thread_result.lock() {
            *result_guard = Some(clipboard_result);
        }
        
        log_println!("[CLIPBOARD] Thread completed in {:?}", clipboard_thread_start.elapsed());
    });
    
    // 在主线程中执行copy操作
    log_println!("[CLIPBOARD] Executing copy in main thread...");
    copy(enigo);
    
    if cancel_select {
        log_println!("[CLIPBOARD] Canceling selection...");
        // 小延迟后取消选择
        thread::sleep(Duration::from_millis(50));
        right_arrow_click(enigo, 1);
    }
    
    // 等待线程完成或超时
    let start_time = Instant::now();
    loop {
        // 检查线程是否已完成
        if let Ok(result_guard) = result.lock() {
            if result_guard.is_some() {
                log_println!("[CLIPBOARD] Thread completed successfully.");
                break;
            }
        }
        
        // 检查是否超时
        if start_time.elapsed().as_millis() > CLIPBOARD_OPERATION_TIMEOUT_MS as u128 {
            log_println!("[CLIPBOARD] Operation timed out after {}ms.", CLIPBOARD_OPERATION_TIMEOUT_MS);
            timeout_flag.store(true, Ordering::SeqCst);
            return Err(GetTextError::Other("Clipboard operation timed out".to_string()));
        }
        
        // 让出CPU时间，避免忙等
        thread::sleep(Duration::from_millis(10));
    }
    
    // 等待线程结束（应该已经完成）
    if let Err(e) = clipboard_thread.join() {
        log_println!("[CLIPBOARD] Thread join error: {:?}", e);
        return Err(GetTextError::Other("Thread join error".to_string()));
    }
    
    // 获取结果
    let final_result = {
        // 在新的作用域中，确保 MutexGuard 尽早释放
        let guard = result.lock().unwrap();
        match &*guard {
            Some(Ok(text)) => Ok(text.clone()),
            Some(Err(e)) => Err(e.clone()),
            None => Err(GetTextError::Other("No result from clipboard thread".to_string())),
        }
    };
    
    final_result
}

// 原函数保持不变，但调用新的带超时版本
pub(crate) fn get_selected_text_by_clipboard(
    enigo: &mut Enigo,
    cancel_select: bool,
) -> Result<String, GetTextError> {
    log_println!("[CLIPBOARD] Called get_selected_text_by_clipboard with cancel_select={}", cancel_select);
    let result = get_selected_text_by_clipboard_internal(enigo, cancel_select);
    log_println!("[CLIPBOARD] get_selected_text_by_clipboard completed with result: {:?}", result.is_ok());
    result
}

pub(crate) fn get_context_via_select_all(
    enigo: &mut Enigo,
    selected_text: &str,
) -> Result<Option<String>, GetTextError> {
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
        return Err(GetTextError::Other("Operation timed out".to_string()));
    }

    // Simulate Ctrl+A (or Cmd+A on macOS)
    log_println!("[SELECT_ALL] Simulating Select All...");
    #[cfg(target_os = "macos")]
    enigo.key(Key::Meta, Direction::Press).unwrap();
    #[cfg(not(target_os = "macos"))]
    enigo.key(Key::Control, Direction::Press).unwrap();

    #[cfg(target_os = "windows")]
    enigo.key(Key::A, Direction::Click).unwrap();
    #[cfg(target_os = "linux")]
    enigo.key(Key::Unicode('a'), Direction::Click).unwrap();
    #[cfg(target_os = "macos")]
    enigo.key(Key::A, Direction::Click).unwrap();

    #[cfg(target_os = "macos")]
    enigo.key(Key::Meta, Direction::Release).unwrap();
    #[cfg(not(target_os = "macos"))]
    enigo.key(Key::Control, Direction::Release).unwrap();
    
    thread::sleep(Duration::from_millis(50)); 
    
    if start_time.elapsed().as_millis() > CLIPBOARD_OPERATION_TIMEOUT_MS as u128 {
        log_println!("[SELECT_ALL] Timeout before Copy. Abort.");
        return Err(GetTextError::Other("Operation timed out".to_string()));
    }

    log_println!("[SELECT_ALL] Simulating Copy...");
    copy(enigo); // Simulate Ctrl+C (or Cmd+C)

    thread::sleep(Duration::from_millis(100)); // Wait for clipboard update
    
    if start_time.elapsed().as_millis() > CLIPBOARD_OPERATION_TIMEOUT_MS as u128 {
        log_println!("[SELECT_ALL] Timeout before getting clipboard content. Abort.");
        return Err(GetTextError::Other("Operation timed out".to_string()));
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
                Err(GetTextError::NotInContext)
            }
        }
        Err(e) => {
            // Failed to get text after Select All + Copy
            log_println!("[SELECT_ALL] Failed to get text from clipboard: {}", e);
            Err(GetTextError::Other("Failed to get text after Select All".to_string()))
        }
    }
}
