use std::fmt::Write;

use crate::{GenerateError, documentation::escape_symbols};

pub fn write(output: &mut String, text: &str, indent: &str) -> Result<(), GenerateError> {
    if text.trim().is_empty() {
        return Ok(());
    }
    let text = escape_symbols(text);
    let text = text.replace('\\', "\\\\").replace('"', "\\\"");
    let mut lines = text.lines();
    if let Some(first) = lines.next() {
        if text.contains('\n') {
            writeln!(output, "{indent}\"\"\"{first}")?;
            for line in lines {
                if line.is_empty() {
                    writeln!(output)?;
                } else {
                    writeln!(output, "{indent}{line}")?;
                }
            }
            writeln!(output, "{indent}\"\"\"")?;
        } else {
            writeln!(output, "{indent}\"\"\"{first}\"\"\"")?;
        }
    }
    Ok(())
}
