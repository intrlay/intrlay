use std::fmt::Write;

use crate::{GenerateError, documentation::escape_symbols};

pub fn write(output: &mut String, text: &str, indent: &str) -> Result<(), GenerateError> {
    if text.trim().is_empty() {
        return Ok(());
    }
    let text = escape_symbols(text);
    for line in text.lines() {
        // C++ removes escaped newlines before parsing comments. Doxygen decodes
        // the entity without letting a trailing backslash consume the next line.
        let line = if line.trim_end().ends_with('\\') {
            let trimmed = line.trim_end();
            format!("{}&#92;", &trimmed[..trimmed.len() - 1])
        } else {
            line.into()
        };
        writeln!(
            output,
            "{indent}///{space}{line}",
            space = crate::documentation::space(&line)
        )?;
    }
    Ok(())
}
