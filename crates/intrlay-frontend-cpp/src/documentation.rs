//! Remove C++ comment delimiters before placing documentation in the shared schema.

use clang::Entity;

pub fn extract(entity: Entity<'_>) -> String {
    let raw = entity.get_comment().unwrap_or_default();
    let raw = raw.trim();
    let block = raw.strip_prefix("/**").or_else(|| raw.strip_prefix("/*!"));
    let text = block.map_or(raw, |body| body.strip_suffix("*/").unwrap_or(body));
    let mut lines = text
        .lines()
        .map(|line| {
            let line = if block.is_some() {
                let trimmed = line.trim_start();
                trimmed.strip_prefix('*').unwrap_or(line)
            } else {
                let trimmed = line.trim_start();
                ["///<", "//!<", "///", "//!"]
                    .iter()
                    .find_map(|prefix| trimmed.strip_prefix(prefix))
                    .unwrap_or(line)
            };
            line.strip_prefix(' ').unwrap_or(line).trim_end()
        })
        .collect::<Vec<_>>();
    while lines.first().is_some_and(|line| line.is_empty()) {
        lines.remove(0);
    }
    while lines.last().is_some_and(|line| line.is_empty()) {
        lines.pop();
    }
    lines.join("\n")
}
