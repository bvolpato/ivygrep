use anyhow::Result;

#[cfg(unix)]
fn reset_sigpipe() {
    unsafe {
        libc::signal(libc::SIGPIPE, libc::SIG_DFL);
    }
}

#[cfg(not(unix))]
fn reset_sigpipe() {}

/// Whether the arguments start the MCP server (`--mcp`, or legacy `mcp serve`).
fn serves_mcp(args: impl Iterator<Item = String>) -> bool {
    let args = args.take_while(|arg| arg != "--").collect::<Vec<_>>();
    args.iter().any(|arg| arg == "--mcp") || args == ["mcp", "serve"]
}

fn main() -> Result<()> {
    reset_sigpipe();
    // The MCP server is blocking stdio code, and each of its daemon requests
    // runs on a short-lived runtime of its own. A worker pool would only add
    // one idle thread per core to every agent session.
    let mut runtime = if serves_mcp(std::env::args().skip(1)) {
        tokio::runtime::Builder::new_current_thread()
    } else {
        tokio::runtime::Builder::new_multi_thread()
    };
    runtime.enable_all().build()?.block_on(ivygrep::cli::run())
}

#[cfg(test)]
mod tests {
    use super::serves_mcp;

    #[test]
    fn only_mcp_invocations_skip_the_worker_pool() {
        let serves = |args: &[&str]| serves_mcp(args.iter().map(ToString::to_string));
        assert!(serves(&["--mcp"]));
        assert!(serves(&["mcp", "serve"]));
        assert!(!serves(&["--daemon"]));
        assert!(!serves(&["mcp"]));
        assert!(!serves(&["--literal", "--", "--mcp"]));
        assert!(!serves(&[]));
    }
}
