fn main() -> anyhow::Result<()> {
    if rovai_computer::native_host::run_if_requested()? {
        return Ok(());
    }
    if rovai_core::interaction_tools::stdio::run_if_requested()? {
        return Ok(());
    }
    if rovai_core::canvas_terminal::run_helper_if_requested()? {
        return Ok(());
    }
    rovai_core::application::run_stdio()
}
