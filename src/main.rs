use anyhow::Result;
use async_trait::async_trait;
use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::get,
    Router,
};
use axum_extra::body::AsyncReadBody;
use config::Config;
use russh::client as SshClient;
use russh_sftp::client::SftpSession;
use serde::Deserialize;
use std::{path::PathBuf, sync::Arc};

#[derive(Deserialize)]
struct SftpServerConfig {
    host: String,
    port: u16,
    username: String,
    password: String,
    path: String,
}

#[derive(Deserialize)]
struct AppConfig {
    port: u16,
    sftp_server: SftpServerConfig,
}

struct Client;

#[async_trait]
impl russh::client::Handler for Client {
    type Error = anyhow::Error;
}

#[tokio::main]
async fn main() {
    let config = Config::builder()
        .add_source(config::Environment::default())
        .build()
        .unwrap();

    let app_config = Arc::new(config.try_deserialize::<AppConfig>().unwrap());

    // build our application with a route
    let app = Router::new().route(
        "/:file",
        get({
            let config = Arc::clone(&app_config);
            move |path| get_file(config, path)
        }),
    );

    // run our app with hyper, listening globally on port 3000
    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", app_config.port))
        .await
        .unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn get_file(app_config: Arc<AppConfig>, file_path: String) -> Response {
    let sftp: SftpSession = match get_sftp_client(app_config.as_ref()).await {
        Ok(sftp) => sftp,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    };

    let mut path = PathBuf::new();
    path.push(app_config.sftp_server.path.as_str());
    path.push(file_path);

    let path = match path.to_str() {
        Some(path) => path,
        None => return StatusCode::BAD_REQUEST.into_response(),
    };

    match sftp.metadata(path).await {
        Ok(f) if f.is_regular() => {}
        _ => return StatusCode::NOT_FOUND.into_response(),
    };

    let f = sftp.open(path).await.unwrap();

    let body = AsyncReadBody::new(f);

    (StatusCode::OK, body).into_response()
}

async fn get_sftp_client(app_config: &AppConfig) -> Result<SftpSession> {
    let config = SshClient::Config::default();
    let sh = Client {};
    let mut session = SshClient::connect(
        Arc::new(config),
        (
            app_config.sftp_server.host.as_str(),
            app_config.sftp_server.port,
        ),
        sh,
    )
    .await?;

    match session
        .authenticate_password(
            app_config.sftp_server.username.as_str(),
            app_config.sftp_server.password.as_str(),
        )
        .await
    {
        Ok(true) => {}
        _ => {
            return Err(anyhow::Error::msg("Authentication Failed"));
        }
    }

    let channel = session.channel_open_session().await?;
    channel.request_subsystem(true, "sftp").await?;
    Ok(SftpSession::new(channel.into_stream()).await?)
}
