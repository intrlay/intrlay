use std::fmt::Write;

use crate::{GenerateError, documentation::escape_symbols};

pub fn write(output: &mut String, text: &str, indent: &str) -> Result<(), GenerateError> {
    if text.trim().is_empty() {
        return Ok(());
    }
    let text = escape_symbols(text);
    writeln!(output, "{indent}/**")?;
    for line in text.lines() {
        let line = line.replace("*/", "*&#47;");
        writeln!(
            output,
            "{indent} *{space}{line}",
            space = crate::documentation::space(&line)
        )?;
    }
    writeln!(output, "{indent} */")?;
    Ok(())
}
