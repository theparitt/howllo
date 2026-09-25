use actix_cors::Cors;
use actix_web::{middleware::Logger, web, App, HttpServer};
use dotenvy::dotenv;
use howllo_server::*;
use tracing::info;
use tracing_subscriber::EnvFilter;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    dotenv().ok();
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let settings = config::Settings::from_env();
    let pool = preflight::run(&settings).await?;
    let pool_data = web::Data::new(pool);
    let settings_data = web::Data::new(settings.clone());
    // Single, process-wide realtime hub shared across all workers.
    let hub_data = web::Data::new(realtime::Hub::new());
    info!(bind_address = %settings.bind_address, "starting server");
    HttpServer::new(move || {
        let cors = settings
            .allowed_origins
            .iter()
            .fold(Cors::default(), |cors, origin| cors.allowed_origin(origin))
            .allow_any_header()
            .allowed_methods(vec!["GET", "POST", "PUT", "PATCH", "DELETE"]);

        App::new()
            .wrap(cors)
            .wrap(Logger::default())
            // An 8 MiB image becomes roughly 10.7 MiB in the JSON base64 upload body.
            .app_data(web::JsonConfig::default().limit(12 * 1024 * 1024))
            .wrap(http::RequestId)
            .wrap(http::PublicWriteRateLimit::new())
            .app_data(pool_data.clone())
            .app_data(settings_data.clone())
            .app_data(hub_data.clone())
            .configure(startup::configure)
    })
    .bind(settings.bind_address)?
    .run()
    .await
}
