//! Source arguments in a compilation database are relative to its command directory.

mod common;

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
        r"using Field = unsigned int;
        struct Container { enum class Value; };
        enum class Container::Value { First = 1, Last = 3 };
        using EnumFromDatabase INTRLAY() = Container::Value;",
    )?;
    fs::write(
        directory.join("source.cpp"),
        "#include <intrlay/annotations.hpp>\n#include <field.hpp>\n\
         #ifndef ENABLE_RECORD\n#error command definitions were lost\n#endif\n\
         struct INTRLAY() FromDatabase { Field value; };\n\
         #ifdef ENABLE_ADAPTER\nstruct Other {};\n\
         template<> struct intrlay::Adapter<Other> { using schema_type = int; };\n#endif\n",
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
            let adapted = extract(
                &source,
                &Options {
                    compilation_database: Some(directory.to_path_buf()),
                    arguments: vec!["-DENABLE_ADAPTER".into()],
                },
            )?;
            assert_eq!(schema, adapted);
            assert!(adapted.exports.contains_key("EnumFromDatabase"));
            assert_eq!(
                schema.exports["FromDatabase"],
                common::named(&schema, "FromDatabase")
            );
            let DefinitionKind::Record(fields) = &common::definition(&schema, "FromDatabase").kind
            else {
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

#[test]
fn forced_include_exports_are_available_to_adapter_probes() -> Result<(), Box<dyn Error>> {
    let temporary = tempfile::tempdir()?;
    let directory = temporary.path();
    fs::write(
        directory.join("prefix.hpp"),
        "#include <intrlay/annotations.hpp>\n\
         struct Owner { enum class Kind; };\n\
         enum class Owner::Kind { First = 1, Last = 3 };\n\
         using Kind INTRLAY() = Owner::Kind;\n\
         struct INTRLAY() Root { Kind kind; };\n",
    )?;
    let source = directory.join("source.cpp");
    fs::write(
        &source,
        "#ifdef ENABLE_ADAPTER\nstruct Other {};\n\
         template<> struct intrlay::Adapter<Other> { using schema_type = int; };\n#endif\n",
    )?;
    let include = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../packages/cpp/include");
    fs::write(
        directory.join("compile_commands.json"),
        serde_json::to_vec(&json!([{
            "directory": directory,
            "file": "source.cpp",
            "arguments": ["clang++", "-std=c++20", format!("-I{}", include.display()),
                "-include", "prefix.hpp", "-c", "source.cpp", "-o", "source.o"]
        }]))?,
    )?;
    let mut options = Options {
        compilation_database: Some(directory.to_owned()),
        ..Options::default()
    };
    let plain = extract(&source, &options)?;
    options.arguments.push("-DENABLE_ADAPTER".into());
    assert_eq!(plain, extract(&source, &options)?);
    assert!(plain.exports.contains_key("Root"));
    assert!(plain.exports.contains_key("Kind"));
    Ok(())
}

#[test]
fn headers_keep_header_semantics_with_warnings_as_errors() -> Result<(), Box<dyn Error>> {
    let temporary = tempfile::tempdir()?;
    let include = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../packages/cpp/include");
    for (filename, language) in [("schema.hpp", None), ("schema.data", Some("c++-header"))] {
        let path = temporary.path().join(filename);
        fs::write(
            &path,
            "#pragma once\n#include <intrlay/annotations.hpp>\nstruct INTRLAY() Root { int value; };\n",
        )?;
        let mut arguments = vec![format!("-I{}", include.display()), "-Werror".into()];
        if let Some(language) = language {
            arguments.extend(["-x".into(), language.into()]);
        }
        let options = Options {
            arguments,
            ..Options::default()
        };
        assert!(extract(path, &options)?.exports.contains_key("Root"));
    }
    Ok(())
}

#[test]
fn parsing_does_not_require_or_write_build_outputs() -> Result<(), Box<dyn Error>> {
    let temporary = tempfile::tempdir()?;
    let directory = temporary.path();
    let source = directory.join("source.cpp");
    fs::write(
        &source,
        "#include <intrlay/annotations.hpp>\nstruct INTRLAY() Root { int value; };\n",
    )?;
    let include = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../packages/cpp/include");
    for output in [
        vec![
            "-o",
            "objects/source.obj",
            "-MMD",
            "-MP",
            "-MF",
            "objects/source.d",
            "-MT",
            "objects/source.obj",
        ],
        vec![
            "-oobjects/source.obj",
            "-MD",
            "-MFobjects/source.d",
            "-MQobjects/source.obj",
            "-MJobjects/source.json",
        ],
    ] {
        let mut arguments = vec![
            "clang++".into(),
            "-std=c++20".into(),
            format!("-I{}", include.display()),
            "-Werror".into(),
        ];
        arguments.extend(output.into_iter().map(str::to_owned));
        arguments.extend(["-c".into(), "--".into(), "source.cpp".into()]);
        fs::write(
            directory.join("compile_commands.json"),
            serde_json::to_vec(&json!([{
                "directory": directory, "file": "source.cpp", "arguments": arguments,
            }]))?,
        )?;
        let options = Options {
            compilation_database: Some(directory.to_owned()),
            ..Options::default()
        };
        assert!(extract(&source, &options)?.exports.contains_key("Root"));
        assert!(!directory.join("objects").exists());
    }
    Ok(())
}

#[test]
#[cfg(windows)]
fn clang_cl_preserves_working_directory_runtime_and_user_diagnostics() -> Result<(), Box<dyn Error>>
{
    let temporary = tempfile::tempdir()?;
    let directory = temporary.path().join("build directory");
    fs::create_dir_all(directory.join("config"))?;
    fs::write(
        directory.join("config/field.hpp"),
        "using Field = unsigned int;\n",
    )?;
    let source = directory.join("source.cpp");
    let prefix = "#include <intrlay/annotations.hpp>\n#include <field.hpp>\n\
        #if !defined(_MT) || !defined(_DLL)\n#error CL runtime flags were lost\n#endif\n\
        struct INTRLAY() Root { Field value; };\n\
        #ifdef ENABLE_ADAPTER\nstruct Other {};\n\
        template<> struct intrlay::Adapter<Other> { using schema_type = int; };\n#endif\n";
    fs::write(&source, prefix)?;
    let include = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../packages/cpp/include");
    fs::write(
        directory.join("compile_commands.json"),
        serde_json::to_vec(&json!([{
            "directory": directory, "file": "source.cpp",
            "arguments": ["clang-cl.exe", "/std:c++20", "-TP", "/W4", "/WX", "-MD",
                format!("/I{}", include.display()), "/Iconfig", "/Foobjects/source.obj", "/Fdobjects/source.pdb",
                "/c", "--", "source.cpp"],
        }]))?,
    )?;
    let mut options = Options {
        compilation_database: Some(directory.clone()),
        ..Options::default()
    };
    let plain = extract(&source, &options)?;
    options.arguments.push("/DENABLE_ADAPTER".into());
    assert_eq!(plain, extract(&source, &options)?);
    assert!(plain.exports.contains_key("Root"));

    options.arguments.push("-fintrlay-invalid-option".into());
    let error = extract(&source, &options)
        .err()
        .ok_or("unknown user flag was accepted under /WX")?;
    assert!(
        error.to_string().contains("intrlay-invalid-option"),
        "{error}"
    );
    options.arguments.pop();
    fs::write(
        &source,
        format!(
            "{prefix}\n[[deprecated(\"keep source warnings\")]] int old_api();\nint test() {{ return old_api(); }}\n"
        ),
    )?;
    let error = extract(&source, &options)
        .err()
        .ok_or("source warning was accepted under /WX")?;
    assert!(
        error.to_string().contains("keep source warnings"),
        "{error}"
    );
    assert!(!directory.join("objects").exists());
    Ok(())
}
