mod config;
mod sftp;

use async_compression::tokio::bufread::BzEncoder;
use axum::{
    extract::Path,
    http::{header, StatusCode},
    response::{IntoResponse, Response, Result},
    routing::get,
    Router,
};
use axum_extra::body::AsyncReadBody;
use camino::Utf8Path;
use config::AppConfig;
use sftp::get_remote_file;
use std::sync::Arc;
use tokio::io::BufReader;

#[tokio::main]
async fn main() {
    let app_config = Arc::new(config::get_app_config());

    // build our application with a route
    let app = Router::new().route(
        "/*file",
        get({
            let config = Arc::clone(&app_config);
            move |path| get_file(path, config)
        }),
    );

    // run our app with hyper, listening globally on port 3000
    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", app_config.port))
        .await
        .unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn get_file(Path(file_path): Path<String>, app_config: Arc<AppConfig>) -> Result<Response> {
    let file_path = Utf8Path::new(file_path.as_str());
    let body = match file_path.extension() {
        Some("bz2") => get_remote_file(&file_path.with_extension(""), app_config.as_ref())
            .await
            .map(|f| AsyncReadBody::new(BzEncoder::new(BufReader::new(f)))),
        _ => get_remote_file(file_path, app_config.as_ref())
            .await
            .map(|f| AsyncReadBody::new(f)),
    }?;

    let headers = [
        (header::CONTENT_TYPE, "application/octet-stream".to_string()),
        (
            header::CONTENT_DISPOSITION,
            format!(
                "attachment; filename=\"{}\"",
                file_path.file_name().unwrap()
            ),
        ),
    ];

    Ok((StatusCode::OK, headers, body).into_response())
}
