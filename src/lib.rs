#[cfg(not(target_os = "macos"))]
mod utils;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "windows")]
mod windows;

#[cfg(target_os = "windows")]
pub use windows::*;

#[derive(Debug, thiserror::Error, Clone)]
pub enum GetTextError {
    #[error("Clipboard error: {0}")]
    Clipboard(String),
    #[error("OS specific error: {0}")]
    Os(String),
    #[error("UIA error: {0}")]
    Uia(String),
    #[error("Input simulation error: {0}")]
    Input(String),
    #[error("Failed to get selected text")]
    NoSelection,
    #[error("Failed to find selection in context")]
    NotInContext,
    #[error("Operation not implemented for this platform yet.")]
    Unimplemented,
    #[error("Other error: {0}")]
    Other(String),
}

/// Gets the selected text using clipboard simulation.
///
/// # Arguments
///
/// * `cancel_select` - If true, simulates a right arrow click after copying to cancel the text selection.
///
/// # Errors
///
/// Returns `GetTextError` if clipboard operations fail or other errors occur.
pub fn get_selected_text(cancel_select: bool) -> Result<String, GetTextError> {
    // 增加调试日志
    #[cfg(target_os = "windows")]
    {
        println!("[LIB] Calling Windows get_selected_text_os with cancel_select={}", cancel_select);
        let result = windows::get_selected_text_os(cancel_select);
        println!("[LIB] Windows get_selected_text_os result: {:?}", result.is_ok());
        result
    }
    #[cfg(target_os = "macos")]
    {
        // macos::get_selected_text_os(cancel_select) // Temporarily disable
        Err(GetTextError::Unimplemented)
    }
    #[cfg(target_os = "linux")]
    {
        // linux::get_selected_text_os(cancel_select) // Temporarily disable
         Err(GetTextError::Unimplemented)
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    {
        Err(GetTextError::Unimplemented)
    }
}

/// Gets the selected text and its surrounding context.
///
/// This function attempts to retrieve the context using platform-specific methods:
/// - Windows: Tries UI Automation first, then falls back to simulating Select All + Copy.
/// - macOS/Linux: Simulates Select All + Copy. (Currently disabled)
///
/// # Arguments
///
/// * `cancel_select` - If true, simulates a right arrow click after copying to cancel the initial text selection.
///
/// # Returns
///
/// Returns a tuple `(selected_text, context_text)` on success.
/// `context_text` might be `None` if context retrieval fails but getting the selection succeeded.
///
/// # Errors
///
/// Returns `GetTextError` if clipboard operations, UIA, or input simulation fail, or if unimplemented.
pub fn get_selected_text_with_context(
    cancel_select: bool,
) -> Result<(String, Option<String>), GetTextError> {
    #[cfg(target_os = "windows")]
    {
        windows::get_selected_text_with_context_os(cancel_select)
    }
    #[cfg(target_os = "macos")]
    {
        // macos::get_selected_text_with_context_os(cancel_select) // Temporarily disable
        Err(GetTextError::Unimplemented)
    }
    #[cfg(target_os = "linux")]
    {
        // linux::get_selected_text_with_context_os(cancel_select) // Temporarily disable
        Err(GetTextError::Unimplemented)
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    {
        Err(GetTextError::Unimplemented)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_selected_text() {
        println!("--- get_selected_text ---");
        let text = get_selected_text(false).unwrap();
        println!("selected text: {:#?}", text);
        println!("--- get_selected_text ---");
        let text = get_selected_text(false).unwrap();
        println!("selected text: {:#?}", text);
        println!("--- get_selected_text ---");
        let text = get_selected_text(false).unwrap();
        println!("selected text: {:#?}", text);
    }

    #[test]
    #[cfg(target_os = "windows")] // 只在 Windows 上运行此测试
    fn test_get_context_windows() {
        use std::{thread, time::Duration};

        println!("\n=== Windows 上下文获取测试 ===");
        println!("请按以下步骤操作：");
        println!("1. 打开记事本或其他文本编辑器（例如记事本、Word等）");
        println!("2. 在编辑器中键入或粘贴一些文字（包含多个句子）");
        println!("3. 选中其中的一小段文字（确保鼠标左键保持按下状态）");
        println!("4. 保持文字选中状态，等待测试自动执行（约10秒）");
        println!("准备开始测试...");
        
        println!("请在 10 秒内切换到目标应用程序并选择一些文本...");
        thread::sleep(Duration::from_secs(10));  // 从5秒增加到10秒

        println!("\n尝试获取选中文本和上下文...");
        let result = get_selected_text_with_context(false); // cancel_select=false 通常用于测试
        println!("调用完成，处理结果中...");
        
        match result {
            Ok((selected, context_opt)) => {
                println!("\n--- 结果 ---");
                println!("选中文本: {:?}", selected);
                match context_opt {
                    Some(context) => {
                        println!("获取到的上下文: {:?}", context);
                        if context.contains(&selected) {
                            println!("✅ 上下文包含选中文本。");
                        } else if selected.split_whitespace().collect::<String>() == context.split_whitespace().collect::<String>() && !context.is_empty() {
                             println!("⚠️ 上下文似乎只等于选中文本（可能UIA只获取到选中部分或Fallback失败）。");
                        }
                         else if !selected.is_empty() {
                             println!("❌ 警告：上下文未包含选中文本！");
                             println!("   (选定: '{}')", selected);
                             println!("   (上下文: '{}')", context);
                         }
                    }
                    None => {
                        if selected.is_empty() {
                             println!("未选中任何文本，也未获取到上下文。");
                        } else {
                             println!("❌ 未能获取上下文 (返回 None)。选中文本是: {:?}", selected);
                        }
                    }
                }
                println!("------------\n");
            }
            Err(e) => {
                eprintln!("\n--- 错误 ---");
                eprintln!("获取文本或上下文时出错: {}", e);
                eprintln!("------------\n");
            }
        }
        
        println!("测试完成。");
    }
}
