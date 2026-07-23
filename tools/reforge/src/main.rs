fn main() -> anyhow::Result<()> {
    reforge::run_from(std::env::args_os())
}
