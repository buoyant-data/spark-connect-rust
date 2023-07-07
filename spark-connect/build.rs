use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_file = "src/gen/spark.connect.rs";

    // Removing the file on every build to make sure that the configured features are respected
    // between invocations
    if std::path::Path::new(&output_file).exists() {
        std::fs::remove_file(output_file)?;
    }

    let tree = "../contrib/spark/connector/connect/common/src/main/protobuf/spark/connect/";
    let protos: Vec<_> = std::fs::read_dir(tree)?
        .map(|e| e.unwrap().path())
        .collect();
    let includes: Vec<PathBuf> =
        vec!["../contrib/spark/connector/connect/common/src/main/protobuf".into()];
    /*
     * The `transport` feature must be disabled for building for wasm targets
     */
    #[cfg(feature = "wasm")]
    let transport = false;
    #[cfg(not(feature = "wasm"))]
    let transport = true;

    tonic_build::configure()
        .build_server(false)
        .build_transport(transport)
        .out_dir("src/gen")
        .compile(&protos, includes.as_slice())?;
    Ok(())
}
