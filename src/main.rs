use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    ivygrep::cli::run().await
}
