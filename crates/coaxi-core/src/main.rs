fn main() -> anyhow::Result<()> {
    if coaxi_core::conversation_tools::stdio::run_if_requested()? {
        return Ok(());
    }
    if coaxi_computer::native_host::run_if_requested()? {
        return Ok(());
    }
    if coaxi_core::interaction_tools::stdio::run_if_requested()? {
        return Ok(());
    }
    if coaxi_core::ui_feedback::stdio::run_if_requested()? {
        return Ok(());
    }
    if coaxi_core::canvas_terminal::run_helper_if_requested()? {
        return Ok(());
    }
    coaxi_core::application::run_stdio()
}
