use std::io::Write;

fn main() -> std::process::ExitCode {
    match coaxi_core::canvas_terminal::run_helper_if_requested() {
        Ok(true) => return std::process::ExitCode::SUCCESS,
        Ok(false) => {}
        Err(error) => {
            let _ = writeln!(std::io::stderr().lock(), "PTY helper failed: {error}");
            return std::process::ExitCode::FAILURE;
        }
    }
    match coaxi_host::run_server() {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            let _ = writeln!(std::io::stderr().lock(), "Coaxi Server failed: {error}");
            std::process::ExitCode::FAILURE
        }
    }
}
