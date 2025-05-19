fn main() -> anyhow::Result<()> {
    tonic_build::compile_protos("../pb-devtools-daemon/proto/daemon.proto")?;
    Ok(())
}
