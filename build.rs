use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let tree = "contrib/spark/connector/connect/common/src/main/protobuf/spark/connect/";
    let protos: Vec<_> = std::fs::read_dir(tree)?
        .map(|e| e.unwrap().path())
        .collect();
    let includes: Vec<PathBuf> =
        vec!["contrib/spark/connector/connect/common/src/main/protobuf".into()];
    tonic_build::configure()
        .build_server(false)
        .out_dir("src/gen")
        .compile(&protos, includes.as_slice())?;
    Ok(())
}
