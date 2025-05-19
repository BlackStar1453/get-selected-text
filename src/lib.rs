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
pub fn get_selected_text() -> Result<String, Box<dyn std::error::Error>> {
    #[cfg(target_os = "windows")]
    {
        let result = windows::get_selected_text();
        println!("[LIB] Windows get_selected_text_os result: {:?}", result.is_ok());
        result.map_err(|e| Box::new(e) as Box<dyn std::error::Error>)
    }
    #[cfg(target_os = "macos")]
    {
        let result = macos::get_selected_text();
        println!("[LIB] macOS get_selected_text_os result: {:?}", result.is_ok());
        result
    }
    #[cfg(target_os = "linux")]
    {
        Err(Box::new(GetTextError::Unimplemented) as Box<dyn std::error::Error>)
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    {
        Err(Box::new(GetTextError::Unimplemented) as Box<dyn std::error::Error>)
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
pub fn get_selected_text_with_context() -> Result<(String, Option<String>), Box<dyn std::error::Error>> {
    #[cfg(target_os = "windows")]
    {
        windows::get_selected_text_with_context_os()
    }
    #[cfg(target_os = "macos")]
    {
        macos::get_selected_text_with_context()
    }
    #[cfg(target_os = "linux")]
    {
        // linux::get_selected_text_with_context_os(_cancel_select) // Temporarily disable
        Err(Box::new(GetTextError::Unimplemented))
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    {
        Err(GetTextError::Unimplemented)
    }
}
