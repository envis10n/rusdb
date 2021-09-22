fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("cargo:rerun-if-changed=proto/rusdb/rusdb.proto");
    tonic_build::configure()
        .build_client(false)
        .build_server(true)
        .compile_well_known_types(true)
        .compile(&["proto/rusdb/rusdb.proto"], &["proto/rusdb"])?;
    Ok(())
}
