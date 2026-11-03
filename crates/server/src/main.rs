#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();
    let _ = rustls::crypto::ring::default_provider().install_default();
    let startup_mode = match std::env::var("CAFE_START_MODE").ok().as_deref() {
        Some("simulation") => Some(true),
        Some("live") => Some(false),
        None => None,
        _ => return Err("CAFE_START_MODE must be simulation or live".into()),
    };
    let simulation =
        startup_mode.unwrap_or_else(|| std::env::var("CAFE_SIMULATION").as_deref() != Ok("0"));
    let home = std::env::var_os("CAFE_HOME")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            std::path::PathBuf::from(
                std::env::var_os("HOME").expect("HOME or CAFE_HOME must be set"),
            )
            .join(".causewaybaycafe")
        });
    let path = std::env::var("CAFE_DATABASE")
        .unwrap_or_else(|_| home.join("cafe.sqlite").to_string_lossy().into_owned());
    let app = cafe_server::init(
        &path,
        &std::env::var("CAFE_OWNER_WALLET").unwrap_or_default(),
        simulation,
    )
    .await?;
    // Explicit Makefile launch modes override the persisted mode once at startup.
    // Owner controls can still change mode while the server is running.
    if let Some(enabled) = startup_mode {
        sqlx::query("UPDATE shop SET simulation=? WHERE id=1")
            .bind(enabled)
            .execute(&app.db)
            .await?;
    }
    sqlx::query("UPDATE shop SET agent_running=0")
        .execute(&app.db)
        .await?;
    sqlx::query("UPDATE order_visuals SET status='paused',error='Server restarted; image was not retried' WHERE status IN ('pending','generating')").execute(&app.db).await?;
    // Recover interrupted image jobs without retrying a possibly billed request automatically.
    sqlx::query("UPDATE image_jobs SET status='paused',error='Server restarted during generation. Review usage before retrying.' WHERE status='generating'").execute(&app.db).await?;
    // Only queued AI work waits one minute. Axum serves requests continuously.
    let worker = app.clone();
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(60)).await;
            if let Err(e) = worker.image_tick().await {
                eprintln!("Image queue: {e}");
            }
            if let Err(e) = worker.flush_journal().await {
                eprintln!("JSONL export pending: {e}");
            }
        }
    });
    let simulator = app.clone();
    sqlx::query("UPDATE service_simulation SET running=0 WHERE id=1")
        .execute(&app.db)
        .await?;
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            if let Err(e) = simulator.service_tick().await {
                eprintln!("Service simulation: {e}");
            }
        }
    });
    let simulation = app.shop().await?["simulation"] == true;
    if let Err(e) = app.flush_journal().await {
        eprintln!("JSONL export pending: {e}");
    }
    let router = cafe_server::router(app);
    if let (Ok(cert), Ok(key)) = (
        std::env::var("CAFE_TLS_CERT"),
        std::env::var("CAFE_TLS_KEY"),
    ) {
        if !cert.is_empty() && !key.is_empty() {
            let r = router.clone();
            tokio::spawn(async move {
                if let Err(e) = cafe_server::transport::serve_h3(r, &cert, &key).await {
                    eprintln!("HTTP/3 failed: {e}");
                }
            });
        }
    }
    let addr = format!(
        "{}:{}",
        std::env::var("CAFE_BIND").unwrap_or("0.0.0.0".into()),
        std::env::var("CAFE_PORT").unwrap_or("8788".into())
    );
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    println!("Causewaybay Cafe http://{addr} · simulation={simulation}");
    axum::serve(listener, router)
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await?;
    Ok(())
}
