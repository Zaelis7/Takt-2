#![forbid(unsafe_code)]

use std::{
    env,
    error::Error,
    fs, io,
    path::{Path, PathBuf},
};

const GENERATED_FILE: &str = "takt.probe.v1.rs";

fn main() -> Result<(), Box<dyn Error>> {
    let command = env::args().nth(1).ok_or_else(|| {
        io::Error::other("usage: cargo run -p xtask -- <generate-proto|check-proto>")
    })?;

    match command.as_str() {
        "generate-proto" => generate_proto(),
        "check-proto" => check_proto(),
        _ => Err(io::Error::other(format!("unknown xtask command: {command}")).into()),
    }
}

fn generate_proto() -> Result<(), Box<dyn Error>> {
    let output_directory = generated_directory();
    fs::create_dir_all(&output_directory)?;
    compile_proto(&output_directory)
}

fn check_proto() -> Result<(), Box<dyn Error>> {
    let temporary_directory = tempfile::tempdir()?;
    compile_proto(temporary_directory.path())?;

    let expected = fs::read(temporary_directory.path().join(GENERATED_FILE))?;
    let committed_path = generated_directory().join(GENERATED_FILE);
    let committed = fs::read(&committed_path).map_err(|error| {
        io::Error::new(
            error.kind(),
            format!(
                "cannot read generated file {}: {error}",
                committed_path.display()
            ),
        )
    })?;

    if expected != committed {
        return Err(io::Error::other(format!(
            "generated Probe types drifted; run `cargo run -p xtask -- generate-proto` ({})",
            committed_path.display()
        ))
        .into());
    }

    Ok(())
}

fn compile_proto(output_directory: &Path) -> Result<(), Box<dyn Error>> {
    let contract = repository_root().join("specs/contracts/probe.proto");
    let include_directory = contract
        .parent()
        .ok_or_else(|| io::Error::other("Probe contract has no parent directory"))?;
    let descriptors = protox::compile([&contract], [include_directory])?;

    let mut configuration = prost_build::Config::new();
    // Preserve the wire contract while avoiding a disproportionately large
    // ServerMessage oneof in generated Rust code.
    configuration.boxed(".takt.probe.v1.ServerMessage.payload.check_job");
    configuration.out_dir(output_directory);
    configuration.compile_fds(descriptors)?;
    Ok(())
}

fn generated_directory() -> PathBuf {
    repository_root().join("crates/probe-protocol/src/generated")
}

fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .map_or_else(|| PathBuf::from("."), Path::to_path_buf)
}
