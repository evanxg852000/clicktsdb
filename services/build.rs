use std::io::Result;

fn main() -> Result<()> {
    prost_build::compile_protos(
        &[
            "src/prometheus/remote/prompb/types.proto",
            "src/prometheus/remote/prompb/remote.proto",
        ],
        &["src/prometheus/remote/prompb/"],
    )?;
    Ok(())
}
