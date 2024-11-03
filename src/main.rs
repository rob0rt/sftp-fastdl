use anyhow::Result;
use async_trait::async_trait;
use axum::{
    extract::Path,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::get,
    Router,
};
use axum_extra::body::AsyncReadBody;
use camino::{Utf8Path, Utf8PathBuf};
use config::Config;
use russh::client as SshClient;
use russh_sftp::{
    client::{self as SftpClient, error::Error as SftpError, SftpSession},
    protocol::StatusCode as SftpStatusCode,
};
use serde::Deserialize;
use tokio::io::BufReader;
use std::sync::Arc;
use async_compression::tokio::bufread::BzEncoder;

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
    sftp: SftpServerConfig,
}

struct Client;

#[async_trait]
impl russh::client::Handler for Client {
    type Error = anyhow::Error;

    async fn check_server_key(
        &mut self,
        _: &russh_keys::key::PublicKey,
    ) -> Result<bool, Self::Error> {
        Ok(true)
    }
}

#[tokio::main]
async fn main() {
    let config = Config::builder()
        .add_source(config::Environment::default().separator("_"))
        .build()
        .unwrap();

    let app_config = Arc::new(config.try_deserialize::<AppConfig>().unwrap());

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

async fn get_file(Path(file_path): Path<String>, app_config: Arc<AppConfig>) -> Response {
    let file_path = Utf8Path::new(file_path.as_str());
    match file_path.extension() {
        Some("bz2") => get_bz2(file_path, app_config.as_ref()).await,
        _ => { 
            let file = match get_remote_file(file_path, app_config.as_ref()).await {
                Ok(f) => f,
                Err(r) => return r,
            };
            let body = AsyncReadBody::new(file);
            (StatusCode::OK, body).into_response()
        },
    }
}

async fn get_bz2(file_path: &Utf8Path, app_config: &AppConfig) -> Response {
    let file = match get_remote_file(&file_path.with_extension(""), app_config).await {
        Ok(f) => f,
        Err(r) => return r,
    };

    let encoder = BzEncoder::new(BufReader::new(file));

    let body = AsyncReadBody::new(encoder);
    
    (StatusCode::OK, body).into_response()
}

async fn get_remote_file(file_path: &Utf8Path, app_config: &AppConfig) -> Result<SftpClient::fs::File, Response> {
    println!("{}", file_path);

    let path = match get_full_path(file_path, &app_config) {
        Ok(p) => p,
        Err(e) => return Err((StatusCode::BAD_REQUEST, e.to_string()).into_response()),
    };

    println!("{}", path);

    let sftp: SftpSession = match get_sftp_client(app_config).await {
        Ok(sftp) => sftp,
        Err(e) => return Err((StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response()),
    };

    match sftp.metadata(&path).await {
        Ok(f) if f.is_regular() => {}
        Err(SftpError::Status(status)) if status.status_code == SftpStatusCode::NoSuchFile => {
            return Err(StatusCode::NOT_FOUND.into_response())
        }
        _ => return Err(StatusCode::BAD_REQUEST.into_response()),
    };

    match sftp.open(&path).await {
        Ok(f) => Ok(f),
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response()),
    }
}

async fn get_sftp_client(app_config: &AppConfig) -> Result<SftpSession> {
    let config = SshClient::Config::default();
    let sh = Client {};
    let mut session = SshClient::connect(
        Arc::new(config),
        (app_config.sftp.host.as_str(), app_config.sftp.port),
        sh,
    )
    .await?;

    match session
        .authenticate_password(
            app_config.sftp.username.as_str(),
            app_config.sftp.password.as_str(),
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

fn get_full_path(file_path: &Utf8Path, app_config: &AppConfig) -> Result<String> {
    let base_path = app_config.sftp.path.as_str();

    let mut path = Utf8PathBuf::new();
    path.push(base_path);
    path.push(file_path);

    // Ensure we didn't do any path traversal trickery
    if !path.starts_with(base_path) {
        return Err(anyhow::Error::msg("Invalid path"));
    }

    Ok(path.into_string())
}
