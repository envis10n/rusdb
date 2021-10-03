use std::process::Command;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Taken from https://github.com/Blightmud/Blightmud build file.
    // taken from https://stackoverflow.com/questions/43753491/include-git-commit-hash-as-string-into-rust-program
    let git_hash = if let Ok(output) = Command::new("git")
        .args(&["rev-parse", "--short", "HEAD"])
        .output()
    {
        String::from_utf8(output.stdout).unwrap_or_default()
    } else {
        String::new()
    };
    println!("cargo:rustc-env=GIT_HASH={}", git_hash);

    let git_tag = if let Ok(output) = Command::new("git")
        .args(&["describe", "--exact-match", "--tags", "HEAD"])
        .output()
    {
        String::from_utf8(output.stdout).unwrap_or_default()
    } else {
        String::new()
    };
    println!("cargo:rustc-env=GIT_TAG={}", git_tag);

    if git_tag.is_empty() {
        let git_describe =
            if let Ok(output) = Command::new("git").args(&["describe", "--tags"]).output() {
                String::from_utf8(output.stdout).unwrap_or_default()
            } else {
                String::new()
            };
        println!(
            "cargo:rustc-env=GIT_DESCRIBE={}",
            format!("({})", git_describe.trim())
        );
    } else {
        println!("cargo:rustc-env=GIT_DESCRIBE=");
    }

    println!("cargo:rerun-if-changed=proto/rusdb/rusdb.proto");
    tonic_build::configure()
        .build_client(false)
        .build_server(true)
        .compile_well_known_types(true)
        .compile(&["proto/rusdb/rusdb.proto"], &["proto/rusdb"])?;
    Ok(())
}
