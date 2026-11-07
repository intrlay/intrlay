//! Source arguments in a compilation database are relative to its command directory.

use std::{error::Error, fs, path::Path};

use intrlay_frontend_cpp::{Options, extract};
use intrlay_schema::{DefinitionKind, Scalar, Type};
use serde_json::json;

#[test]
fn compilation_database_resolves_source_spellings() -> Result<(), Box<dyn Error>> {
    let temporary = tempfile::tempdir()?;
    let directory = temporary.path();
    fs::create_dir(directory.join("include"))?;
    fs::create_dir(directory.join("nested"))?;
    fs::write(
        directory.join("include/field.hpp"),
        "using Field = unsigned int;",
    )?;
    fs::write(
        directory.join("source.cpp"),
        "#include <intrlay/annotations.hpp>\n#include <field.hpp>\n\
         #ifndef ENABLE_RECORD\n#error command definitions were lost\n#endif\n\
         struct INTRLAY() FromDatabase { Field value; };\n",
    )?;
    let include = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../packages/cpp/include");
    let source = directory.join("source.cpp");
    let absolute = source.to_string_lossy().into_owned();
    for file in [absolute.as_str(), "source.cpp"] {
        for argument in [absolute.as_str(), "source.cpp", "./nested/../source.cpp"] {
            fs::write(
                directory.join("compile_commands.json"),
                serde_json::to_vec(&json!([{
                    "directory": directory,
                    "file": file,
                    "arguments": ["clang++", "-std=c++20", format!("-I{}", include.display()),
                        "-Iinclude", "-DENABLE_RECORD", "-c", argument, "-o", "source.o"]
                }]))?,
            )?;
            let schema = extract(
                &source,
                &Options {
                    compilation_database: Some(directory.to_path_buf()),
                    ..Options::default()
                },
            )?;
            let DefinitionKind::Record(fields) = &schema.definitions["FromDatabase"].kind else {
                return Err("record expected".into());
            };
            assert_eq!(fields[0].ty, Type::Scalar(Scalar::U32));
        }
    }
    Ok(())
}

#[test]
fn relative_requested_source_survives_compiler_working_directory() -> Result<(), Box<dyn Error>> {
    let temporary = tempfile::tempdir()?;
    let source = Path::new("tests/fixtures/redeclarations.hpp");
    let absolute = std::path::absolute(source)?;
    let directory = absolute.parent().ok_or("source directory expected")?;
    let include = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../packages/cpp/include");
    fs::write(
        temporary.path().join("compile_commands.json"),
        serde_json::to_vec(&json!([{
            "directory": directory,
            "file": absolute,
            "arguments": ["clang++", "-std=c++20", format!("-I{}", include.display()),
                "-c", "redeclarations.hpp", "-o", "source.o"]
        }]))?,
    )?;
    let schema = extract(
        source,
        &Options {
            compilation_database: Some(temporary.path().to_path_buf()),
            ..Options::default()
        },
    )?;
    assert!(schema.exports.contains_key("Root"));
    Ok(())
}
