//! Process entrypoint. All wiring lives in [`ob::app`].

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    ob::app::run().await
}
