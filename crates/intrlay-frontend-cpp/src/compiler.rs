//! Adapt native build commands for parsing without producing build artifacts.

use std::path::{Path, PathBuf};

use clang::{CompilationDatabase, diagnostic::Diagnostic};

use crate::{FrontendError, Options};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Driver {
    Gnu,
    Cl,
}

impl Driver {
    fn detect(executable: &str, arguments: &[String]) -> Self {
        if let Some(mode) = arguments
            .iter()
            .rev()
            .find_map(|arg| arg.strip_prefix("--driver-mode="))
        {
            return if mode == "cl" { Self::Cl } else { Self::Gnu };
        }
        let name = executable.rsplit(['/', '\\']).next().unwrap_or(executable);
        if ["cl", "cl.exe", "clang-cl", "clang-cl.exe"]
            .iter()
            .any(|candidate| name.eq_ignore_ascii_case(candidate))
        {
            Self::Cl
        } else {
            Self::Gnu
        }
    }

    fn option(self, gnu: &str) -> String {
        match self {
            Self::Gnu => gnu.into(),
            Self::Cl => format!("/clang:{gnu}"),
        }
    }
}

pub fn arguments(path: &Path, options: &Options) -> Result<Vec<String>, FrontendError> {
    let mut executable = String::new();
    let mut supplied = Vec::new();
    let mut directory = None;
    let mut source = None;
    if let Some(database_directory) = &options.compilation_database {
        let database = CompilationDatabase::from_directory(database_directory)
            .map_err(|error| FrontendError(format!("compilation database: {error:?}")))?;
        let commands = database.get_compile_commands(path).map_err(|error| {
            FrontendError(format!("compile command for {}: {error:?}", path.display()))
        })?;
        let commands = commands.get_commands();
        let command = commands
            .first()
            .ok_or_else(|| FrontendError("empty compile command".into()))?;
        let working_directory = command.get_directory();
        source = Some(resolved_source(&command.get_filename(), &working_directory));
        directory = Some(working_directory);
        let mut arguments = command.get_arguments().into_iter();
        executable = arguments.next().unwrap_or_default();
        supplied.extend(arguments);
    }
    supplied.extend(options.arguments.iter().cloned());
    let driver = Driver::detect(&executable, &supplied);
    let mut result = Vec::new();
    if driver == Driver::Cl {
        result.push("--driver-mode=cl".into());
    }

    // Respect explicit language/standard flags; defaults must not conflict with
    // CMake's /TP or turn an ordinary header into a warning-producing main file.
    if !supplied
        .iter()
        .any(|argument| has_language(argument, driver))
    {
        let header = path
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| {
                matches!(
                    extension.to_ascii_lowercase().as_str(),
                    "h" | "hh" | "hpp" | "hxx" | "inc"
                )
            });
        if driver == Driver::Cl && !header {
            result.push("/TP".into());
        } else {
            result.push(driver.option(if header { "-xc++-header" } else { "-xc++" }));
        }
    }
    if !supplied.iter().any(|argument| {
        argument.starts_with("-std=")
            || argument.starts_with("/std:")
            || argument.starts_with("-std:")
            || argument.starts_with("/clang:-std=")
    }) {
        result.push(match driver {
            Driver::Gnu => "-std=c++20".into(),
            Driver::Cl => "/std:c++20".into(),
        });
    }
    if let Some(directory) = &directory {
        if driver == Driver::Cl {
            // clang-cl's driver option alone does not set libclang's frontend
            // filesystem directory. Forward it to cc1 without changing cwd.
            result.extend([
                "-Xclang".into(),
                "-working-directory".into(),
                "-Xclang".into(),
                directory.display().to_string(),
            ]);
        } else {
            result.push(format!("-working-directory={}", directory.display()));
        }
    }
    let input = path.canonicalize().unwrap_or_else(|_| path.to_owned());
    let mut arguments = supplied.into_iter();
    while let Some(argument) = arguments.next() {
        if let Some(operand) = output_option(&argument, driver) {
            if operand {
                arguments.next();
            }
            continue;
        }
        if argument == "--" || argument == "-c" || (driver == Driver::Cl && argument == "/c") {
            continue;
        }
        if let Some(directory) = &directory
            && !argument.starts_with('-')
        {
            let candidate = resolved_source(Path::new(&argument), directory);
            if Some(&candidate) == source.as_ref() || candidate == input {
                continue;
            }
        }
        result.push(argument);
    }
    Ok(result)
}

fn has_language(argument: &str, driver: Driver) -> bool {
    argument.starts_with("-x")
        || argument.starts_with("/clang:-x")
        || (driver == Driver::Cl && matches!(argument, "/TP" | "-TP" | "/TC" | "-TC"))
}

/// `Some(true)` consumes a separate output filename; attached values consume none.
fn output_option(argument: &str, driver: Driver) -> Option<bool> {
    if argument.starts_with("-o") {
        return Some(argument == "-o");
    }
    match driver {
        Driver::Gnu => {
            if matches!(argument, "-M" | "-MM" | "-MD" | "-MMD" | "-MP" | "-MG") {
                return Some(false);
            }
            for option in ["-MF", "-MT", "-MQ", "-MJ"] {
                if argument.starts_with(option) {
                    return Some(argument == option);
                }
            }
        }
        Driver::Cl => {
            // -MD is the CL runtime choice, not GNU dependency generation.
            let argument = argument
                .strip_prefix('/')
                .or_else(|| argument.strip_prefix('-'))?;
            for option in ["Fo", "Fd", "Fe", "Fa", "Fi", "sourceDependencies"] {
                if argument.starts_with(option) {
                    return Some(argument == option);
                }
            }
        }
    }
    None
}

fn resolved_source(path: &Path, directory: &Path) -> PathBuf {
    let path = directory.join(path);
    path.canonicalize().unwrap_or(path)
}

pub fn is_parser_option_diagnostic(diagnostic: &Diagnostic<'_>) -> bool {
    // libclang appends these GNU-only tooling flags even in clang-cl mode.
    // They do not affect the schema AST. Ignore only their driver diagnostics;
    // /WX still applies to user flags and every diagnostic located in source.
    diagnostic.get_location().get_file_location().file.is_none()
        && matches!(
            diagnostic.get_text().as_str(),
            "unknown argument ignored in clang-cl: '-fno-spell-checking'"
                | "unknown argument ignored in clang-cl: '-fallow-editor-placeholders'"
        )
}

#[cfg(test)]
mod tests {
    use super::{Driver, output_option};

    #[test]
    fn output_flags_follow_the_selected_driver() {
        assert_eq!(output_option("-MD", Driver::Gnu), Some(false));
        assert_eq!(output_option("-MD", Driver::Cl), None);
        assert_eq!(output_option("/MD", Driver::Cl), None);
        assert_eq!(output_option("-MF", Driver::Gnu), Some(true));
        assert_eq!(
            output_option("-MFobjects/source.d", Driver::Gnu),
            Some(false)
        );
        assert_eq!(output_option("/Fo", Driver::Cl), Some(true));
        assert_eq!(
            output_option("-Foobjects/source.obj", Driver::Cl),
            Some(false)
        );
    }
}
