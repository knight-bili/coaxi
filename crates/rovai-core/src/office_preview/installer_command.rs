// Adapted from Codeg aff1e6c, Apache-2.0; see THIRD_PARTY_NOTICES.
use std::time::Duration;
const OFFICECLI_MANUAL_URL: &str = "https://github.com/iOfficeAI/OfficeCLI";
const OFFICECLI_INSTALL_SH_MIRROR_URL: &str = "https://d.officecli.ai/install.sh";
const OFFICECLI_INSTALL_SH_GITHUB_URL: &str =
    "https://raw.githubusercontent.com/iOfficeAI/OfficeCLI/main/install.sh";
const OFFICECLI_INSTALL_PS1_MIRROR_URL: &str = "https://d.officecli.ai/install.ps1";
const OFFICECLI_INSTALL_PS1_GITHUB_URL: &str =
    "https://raw.githubusercontent.com/iOfficeAI/OfficeCLI/main/install.ps1";

/// Hard ceiling on the whole installer subprocess — the small script download
/// *and* the multi-MB binary the script then fetches. Generous (slow networks
/// pulling tens of MB are fine) but bounded: a stalled mirror or hung download
/// must never pin the mutation lock and the UI's "installing" state forever. On
/// timeout the whole process tree is killed (see `stream_install_or_kill_tree`).
/// The per-request `curl`/`irm` timeouts below only bound the script fetch; this
/// bounds everything.
const OFFICECLI_INSTALL_TIMEOUT: Duration = Duration::from_secs(600);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InstallOs {
    Unix,
    Windows,
}

/// `cfg!(windows)` (not `#[cfg]`) so both variants stay referenced in source —
/// otherwise the unused variant trips dead-code on the platform that omits it.
fn current_install_os() -> InstallOs {
    if cfg!(windows) {
        InstallOs::Windows
    } else {
        InstallOs::Unix
    }
}

struct OfficecliInstallCommand {
    program: String,
    args: Vec<String>,
}

/// Build the installer invocation for `os`. Both branches try the mirror first,
/// then fall back to GitHub raw. Kept platform-parameterized (not `cfg`-gated)
/// so unit tests verify both shapes on any host.
fn officecli_install_command(os: InstallOs) -> OfficecliInstallCommand {
    match os {
        InstallOs::Windows => OfficecliInstallCommand {
            program: "powershell.exe".to_string(),
            args: vec![
                "-NoProfile".to_string(),
                "-ExecutionPolicy".to_string(),
                "Bypass".to_string(),
                "-Command".to_string(),
                // Hardening preamble. `iex $s` runs the vendor install.ps1 in THIS
                // same process/scope, so these settings also govern the multi-MB
                // binary download the script does via `Invoke-WebRequest`:
                //   • TLS 1.2 — Windows PowerShell 5.1 on older/locked-down .NET
                //     can default to TLS 1.0, which GitHub (and most CDNs) reject.
                //     Add it ADDITIVELY (`-bor`, so existing protocols are kept)
                //     and ONLY when the current value is not `SystemDefault`
                //     (value 0): a modern host that lets the OS negotiate TLS 1.3
                //     is left exactly as-is — no regression. Compare against `0`,
                //     not the `SystemDefault` enum member (added in .NET 4.7;
                //     referencing it throws on 4.5/4.6 and the catch would then
                //     skip the upgrade on exactly the old hosts that need it).
                //   • `$ProgressPreference` — silence `Invoke-WebRequest`'s progress
                //     rendering, which slows the binary download by orders of
                //     magnitude on PS 5.1 (and is just noise when stdout is piped).
                //   • `[Console]::OutputEncoding` — emit UTF-8 (no BOM) so non-ASCII
                //     installer/error text (OEM codepage on non-English Windows)
                //     decodes in our line reader. Best-effort (the setter can throw
                //     with no console attached); the lossy reader is the real net.
                //   • `-TimeoutSec` bounds each script fetch so a stalled mirror
                //     fails over to GitHub instead of hanging; the binary download
                //     the script then does is bounded by OFFICECLI_INSTALL_TIMEOUT.
                format!(
                    "$ErrorActionPreference='Stop'; try {{ $sp=[Net.ServicePointManager]::SecurityProtocol; if([int]$sp -ne 0){{ [Net.ServicePointManager]::SecurityProtocol=$sp -bor [Net.SecurityProtocolType]::Tls12 }} }} catch {{}}; $ProgressPreference='SilentlyContinue'; try {{ [Console]::OutputEncoding=[System.Text.UTF8Encoding]::new($false) }} catch {{}}; try {{ $s = irm -TimeoutSec 60 {OFFICECLI_INSTALL_PS1_MIRROR_URL} }} catch {{ $s = irm -TimeoutSec 60 {OFFICECLI_INSTALL_PS1_GITHUB_URL} }}; iex $s"
                ),
            ],
        },
        InstallOs::Unix => OfficecliInstallCommand {
            program: "bash".to_string(),
            args: vec![
                "-lc".to_string(),
                // Download to a temp file rather than `curl | bash`: a
                // connection dropped mid-stream would otherwise concatenate the
                // fallback output after a partial script. The `--connect-timeout`
                // / `--max-time` flags bound each script fetch so a stalled
                // mirror fails over to GitHub instead of hanging; the binary
                // download the script then does is bounded by
                // OFFICECLI_INSTALL_TIMEOUT.
                format!(
                    "f=$(mktemp) || exit 1; (curl -fsSL --connect-timeout 20 --max-time 60 {OFFICECLI_INSTALL_SH_MIRROR_URL} -o \"$f\" || curl -fsSL --connect-timeout 20 --max-time 60 {OFFICECLI_INSTALL_SH_GITHUB_URL} -o \"$f\") && bash \"$f\"; s=$?; rm -f \"$f\"; exit $s"
                ),
            ],
        },
    }
}

