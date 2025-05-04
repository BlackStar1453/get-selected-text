use enigo::*;
use crate::utils::*{
    get_selected_text_by_clipboard,
    get_context_via_select_all,
};
use crate::GetTextError;
use std::{thread, time::Duration};

pub fn get_selected_text() -> Result<String, Box<dyn std::error::Error>> {
    let mut enigo = Enigo::new(&Settings::default()).unwrap();
    crate::utils::get_selected_text_by_clipboard(&mut enigo, false)
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

    // 2. On Linux, directly use the fallback: Select All + Copy
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
