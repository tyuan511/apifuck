use anyhow::Result;
#[tokio::main]
async fn main() -> Result<()> {
    let port = std::env::var("APIFUCK_DEMO_PORT")
        .unwrap_or_else(|_| "8787".into())
        .parse::<u16>()?;
    let listener = tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, port)).await?;
    let address = listener.local_addr()?;
    let base_url = format!("http://{address}");
    let ws_url = format!("ws://{address}/socket");
    if let Some(workspace) = std::env::var_os("APIFUCK_DEMO_WORKSPACE") {
        apifuck_demo::configure_workspace(std::path::Path::new(&workspace), &base_url, &ws_url)?;
    }
    println!(
        "{}",
        serde_json::json!({"service":"apifuck-demo","base_url":base_url,"ws_url":ws_url})
    );
    axum::serve(listener, apifuck_demo::router())
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await?;
    Ok(())
}
