fn main() -> anyhow::Result<()> {
    if coaxi_core::canvas_terminal::run_helper_if_requested()? {
        return Ok(());
    }
    coaxi_host::run_host()
}
