fn main() {
    if std::env::args().any(|arg| arg == "--mcp") {
        if let Err(error) = anubis_engine_lib::mcp::run_stdio() {
            tracing::error!("anubis MCP server failed: {}", error);
            std::process::exit(1);
        }
        return;
    }

    anubis_engine_lib::run();
}
