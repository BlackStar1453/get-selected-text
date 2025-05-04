use crate::utils::*;
use crate::GetTextError;
use enigo::{Enigo, Settings};
use uiautomation::UIAutomation;
use uiautomation::patterns::UITextPattern;
use uiautomation::types::TextUnit;
use std::{thread, time::Duration};

// Use debug_print for logging if enabled, otherwise println
#[cfg(debug_assertions)]
use debug_print::debug_println as log_println;
#[cfg(not(debug_assertions))]
use println as log_println;

const CONTEXT_CHARS_BEFORE_UIA_FALLBACK: usize = 150;
const CONTEXT_CHARS_AFTER_UIA_FALLBACK: usize = 150;

pub fn get_selected_text_os(cancel_select: bool) -> Result<String, GetTextError> {
    log_println!("[GET_TEXT_OS] Starting get_selected_text_os...");
    let mut enigo = Enigo::new(&Settings::default()).map_err(|e| GetTextError::Input(e.to_string()))?;
    
    // 使用原有的 get_selected_text_by_clipboard 函数获取选中文本
    log_println!("[GET_TEXT_OS] Getting selected text via clipboard...");
    let result = get_selected_text_by_clipboard(&mut enigo, cancel_select);
    log_println!("[GET_TEXT_OS] get_selected_text_by_clipboard result: {:?}", result.is_ok());
    
    result
}

pub fn get_selected_text_with_context_os(
    cancel_select: bool,
) -> Result<(String, Option<String>), GetTextError> {
    log_println!("[CTX_OS] Starting get_selected_text_with_context_os...");
    
    // 1. 调用现有的 get_selected_text 函数获取选中文本
    log_println!("[CTX_OS] Calling get_selected_text...");
    let selected_text = crate::get_selected_text(cancel_select)?;
    log_println!("[CTX_OS] Initial selected text: {:?}", selected_text);

    if selected_text.is_empty() {
        log_println!("[CTX_OS] Selected text is empty, returning early.");
        return Ok((selected_text, None));
    }

    // 初始化 Enigo，用于后续的上下文获取
    let mut enigo = Enigo::new(&Settings::default()).map_err(|e| GetTextError::Input(e.to_string()))?;

    // 2. Try getting context using UIA
    log_println!("[CTX_OS] Attempting UIA context retrieval...");
    match get_context_via_uia(&selected_text) {
        Ok(Some(context)) => {
            log_println!("[CTX_OS] UIA context retrieval successful.");
            return Ok((selected_text, Some(context)));
        }
        Ok(None) => {
            log_println!("[CTX_OS] UIA context retrieval ran but found no context.");
        }
        Err(e) => {
            log_println!("[CTX_OS] UIA context retrieval failed: {}, falling back...", e);
        }
    }

    // 3. Fallback: Try getting context using Select All + Copy
    log_println!("[CTX_OS] Attempting fallback context retrieval (Select All + Copy)...");
    // Short delay before fallback simulation to avoid race conditions
    thread::sleep(Duration::from_millis(100));
    let fallback_result = get_context_via_select_all(&mut enigo, &selected_text);
    log_println!("[CTX_OS] Fallback result: {:?}", fallback_result.is_ok());

    match fallback_result {
        Ok(Some(context)) => Ok((selected_text, Some(context))),
        Ok(None)=> Ok((selected_text, None)), // Should not happen if selected_text is not empty
        Err(GetTextError::NotInContext) => {
            log_println!("[CTX_OS] Fallback failed: Selected text not found in full text.");
            Ok((selected_text, None)) 
        }
        Err(e) => {
             log_println!("[CTX_OS] Fallback context retrieval failed: {}", e);
             Ok((selected_text, None))
        } 
    }
}

fn get_context_via_uia(selected_text_clipboard: &str) -> Result<Option<String>, GetTextError> {
    log_println!("[UIA] Starting get_context_via_uia...");
    let automation = UIAutomation::new().map_err(|e| {
        log_println!("[UIA] Failed to create UIAutomation instance: {}", e);
        GetTextError::Uia(e.to_string())
    })?;
    
    log_println!("[UIA] Getting focused element...");
    let Ok(focused_element) = automation.get_focused_element() else {
         log_println!("[UIA] Failed to get focused element.");
         return Err(GetTextError::Uia("Failed to get focused element".to_string()));
    };
    let focused_runtime_id = focused_element.get_runtime_id().unwrap_or_default();
     log_println!("[UIA] Focused element RuntimeId: {:?}", focused_runtime_id);

    log_println!("[UIA] Getting control view walker...");
    let walker = automation.get_control_view_walker().map_err(|e| {
        log_println!("[UIA] Failed to get control view walker: {}", e);
        GetTextError::Uia(format!("Failed to get control view walker: {}", e))
    })?;

    log_println!("[UIA] Starting parent traversal loop...");
    let mut current_element_opt = Ok(focused_element);
    let mut loop_count = 0; // Limit loop iterations for safety
    const MAX_LOOP_COUNT: u32 = 20; 

    loop {
        if loop_count >= MAX_LOOP_COUNT {
            log_println!("[UIA] Loop limit reached ({}), stopping parent traversal.", MAX_LOOP_COUNT);
            break;
        }
        loop_count += 1;

        let Ok(current_element) = current_element_opt else {
            log_println!("[UIA] Error during element navigation, stopping loop.");
            break; // Error occurred during navigation
        };
        let current_runtime_id = current_element.get_runtime_id().unwrap_or_default();
        log_println!("[UIA] Loop #{}: Checking element RuntimeId: {:?}", loop_count, current_runtime_id);

        // Try to get the TextPattern
        log_println!("[UIA] Loop #{}: Attempting to get TextPattern...", loop_count);
        match current_element.get_pattern::<UITextPattern>() {
            Ok(pattern) => {
                log_println!("[UIA] Loop #{}: TextPattern found! Processing...", loop_count);
                match process_text_pattern(&pattern, selected_text_clipboard) {
                    Ok(Some(context)) => {
                        log_println!("[UIA] Loop #{}: Context found via TextPattern!", loop_count);
                        return Ok(Some(context)); // Found context
                    }
                    Ok(None) => { 
                         log_println!("[UIA] Loop #{}: Pattern processed, but no matching context found.", loop_count);
                         /* Pattern processed, but no matching selection/context */ 
                    }
                    Err(e) => {
                         log_println!("[UIA] Loop #{}: Error processing TextPattern: {}", loop_count, e);
                         return Err(e); // Error during pattern processing
                    }
                }
            }
            Err(_) => {
                 log_println!("[UIA] Loop #{}: TextPattern not found for this element.", loop_count);
                 // Pattern not available for this element
            }
        }
        
        // Navigate to parent using the correct method name
         log_println!("[UIA] Loop #{}: Attempting to get parent element...", loop_count);
        current_element_opt = walker.get_parent(&current_element)
                                  .map_err(|e| {
                                        log_println!("[UIA] Loop #{}: Failed to get parent element: {}", loop_count, e);
                                        GetTextError::Uia(format!("Failed to get parent element: {}", e))
                                  });
        
        // Break if get_parent_element returns an error (likely no more parents or other issue)
        if current_element_opt.is_err() { 
             log_println!("[UIA] Loop #{}: Error getting parent, stopping loop.", loop_count);
            break;
        }
    }

    log_println!("[UIA] Parent traversal loop finished. UIA did not find context.");
    Ok(None)
}

fn process_text_pattern(pattern: &UITextPattern, selected_text_clipboard: &str) -> Result<Option<String>, GetTextError> {
    log_println!("[UIA_PATTERN] Starting process_text_pattern...");
    
    log_println!("[UIA_PATTERN] Getting selection...");
    let selection = pattern.get_selection().map_err(|e| GetTextError::Uia(format!("Failed to get selection: {}", e)))?;    
    if selection.is_empty() {
         log_println!("[UIA_PATTERN] No selection found in pattern.");
        return Ok(None);
    }
    log_println!("[UIA_PATTERN] Selection found ({} ranges).", selection.len());

    let text_range = &selection[0]; // Use the first selection range
    log_println!("[UIA_PATTERN] Getting text from first selection range...");
    let selected_text_uia = text_range.get_text(-1).map_err(|e| GetTextError::Uia(format!("Failed to get text from range: {}", e)))?;
    log_println!("[UIA_PATTERN] Text from UIA range: {:?}", selected_text_uia);

    // Normalize whitespace for comparison (optional, but might help)
    let norm_clipboard = selected_text_clipboard.split_whitespace().collect::<String>();
    let norm_uia = selected_text_uia.split_whitespace().collect::<String>();
    log_println!("[UIA_PATTERN] Comparing UIA text ({:?}) with clipboard text ({:?})...", norm_uia, norm_clipboard);

    // Check if the text from UIA matches (or contains/is contained by) the clipboard text
    if norm_uia.contains(&norm_clipboard) || norm_clipboard.contains(&norm_uia) {
        log_println!("[UIA_PATTERN] Match found! Attempting context expansion...");
        
        // Remove the attempt to expand to Sentence as it's causing errors
        // ... (Sentence code removed previously)

        // Attempt to expand to Paragraph instead
        log_println!("[UIA_PATTERN] Attempting to expand to Paragraph...");
        let expanded_range_para = text_range.clone(); // 移除 mut 关键字，因为变量没有被修改
        if expanded_range_para.expand_to_enclosing_unit(TextUnit::Paragraph).is_ok() {
             log_println!("[UIA_PATTERN] Expanded to Paragraph successfully. Getting text...");
            if let Ok(paragraph_text) = expanded_range_para.get_text(-1) {
                log_println!("[UIA_PATTERN] Paragraph text: {:?}", paragraph_text);
                if paragraph_text.contains(&selected_text_uia) {
                     log_println!("[UIA_PATTERN] Context found via Paragraph expansion.");
                     return Ok(Some(paragraph_text));
                }
            }
        } else {
             log_println!("[UIA_PATTERN] Failed to expand to Paragraph.");
        }

        // Fallback: Get full document text and extract context manually
        log_println!("[UIA_PATTERN] Attempting fallback: getting document range...");
        if let Ok(doc_range) = pattern.get_document_range() {
            log_println!("[UIA_PATTERN] Getting text from document range...");
            if let Ok(full_text) = doc_range.get_text(-1) {
                 log_println!("[UIA_PATTERN] Full document text length: {}", full_text.len());
                if let Some(start_pos) = full_text.find(&selected_text_uia) {
                    log_println!("[UIA_PATTERN] Found UIA selection within full text. Extracting context...");
                    let end_pos = start_pos + selected_text_uia.len();
                    let context_start = start_pos.saturating_sub(CONTEXT_CHARS_BEFORE_UIA_FALLBACK);
                    let context_end = (end_pos + CONTEXT_CHARS_AFTER_UIA_FALLBACK).min(full_text.len());
                    
                    // Ensure valid UTF-8 boundaries
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
                         log_println!("[UIA_PATTERN] Context found via document range fallback.");
                         return Ok(Some(context));
                    } else {
                        log_println!("[UIA_PATTERN] Failed to get valid context boundaries from full text. Returning full text.");
                        return Ok(Some(full_text)); // Return full text if boundaries fail
                    }
                } else {
                     log_println!("[UIA_PATTERN] UIA selection not found within full document text.");
                }
            } else {
                log_println!("[UIA_PATTERN] Failed to get text from document range.");
            }
        } else {
             log_println!("[UIA_PATTERN] Failed to get document range.");
        }
        
        log_println!("[UIA_PATTERN] All expansion/fallback failed. Returning UIA selection as context.");
        return Ok(Some(selected_text_uia)); // Return UIA selection as context
    } else {
         log_println!("[UIA_PATTERN] UIA selection did not match clipboard text.");
    }

    log_println!("[UIA_PATTERN] No context found in this pattern.");
    Ok(None)
}
