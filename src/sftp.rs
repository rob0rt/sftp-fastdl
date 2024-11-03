use std::{fmt::Display, sync::Arc};

use async_trait::async_trait;
use axum::{http::StatusCode, response::IntoResponse};
use camino::{Utf8Component, Utf8Path, Utf8PathBuf};
use russh::{client as SshClient, Error as SshError};
use russh_sftp::{
    client::{self as SftpClient, error::Error as SftpError, SftpSession},
    protocol::StatusCode as SftpStatusCode,
};

use crate::config::AppConfig;

pub enum RemoteFileError {
    PathTraversal,
    SftpClientError(SftpClientError),
    FileNotFound,
    FileAccessError(SftpError),
}

impl From<SftpClientError> for RemoteFileError {
    fn from(value: SftpClientError) -> Self {
        Self::SftpClientError(value)
    }
}

impl IntoResponse for RemoteFileError {
    fn into_response(self) -> axum::response::Response {
        match self {
            Self::PathTraversal => {
                (StatusCode::BAD_REQUEST, "Request contains path traversal").into_response()
            }
            Self::SftpClientError(e) => {
                (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response()
            }
            Self::FileNotFound => (StatusCode::NOT_FOUND, "File not found").into_response(),
            Self::FileAccessError(error) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Error accessing file: {}", error),
            )
                .into_response(),
        }
    }
}

pub async fn get_remote_file(
    file_path: &Utf8Path,
    app_config: &AppConfig,
) -> Result<SftpClient::fs::File, RemoteFileError> {
    // Ensure we don't do any path traversal trickery
    if file_path
        .components()
        .any(|c| c == Utf8Component::ParentDir)
    {
        return Err(RemoteFileError::PathTraversal);
    }

    // Append the requested file path to the sftp base path
    let mut path = Utf8PathBuf::new();
    path.push(app_config.sftp.path.as_str());
    path.push(file_path);
    let path = path.into_string();

    let sftp = get_sftp_client(app_config).await?;

    match sftp.metadata(&path).await {
        Ok(f) if f.is_regular() => {}
        Ok(_) => return Err(RemoteFileError::FileNotFound),
        Err(SftpError::Status(status)) if status.status_code == SftpStatusCode::NoSuchFile => {
            return Err(RemoteFileError::FileNotFound)
        }
        Err(e) => return Err(RemoteFileError::FileAccessError(e)),
    };

    match sftp.open(&path).await {
        Ok(f) => Ok(f),
        Err(e) => Err(RemoteFileError::FileAccessError(e)),
    }
}

struct Client;

#[async_trait]
impl russh::client::Handler for Client {
    type Error = SshError;

    async fn check_server_key(
        &mut self,
        _: &russh_keys::key::PublicKey,
    ) -> Result<bool, Self::Error> {
        Ok(true)
    }
}

pub enum SftpClientError {
    SshError(SshError),
    SshAuthFailure,
    SftpError(SftpError),
}

impl From<SshError> for SftpClientError {
    fn from(value: SshError) -> Self {
        Self::SshError(value)
    }
}

impl From<SftpError> for SftpClientError {
    fn from(value: SftpError) -> Self {
        Self::SftpError(value)
    }
}

impl Display for SftpClientError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SshError(error) => write!(f, "ssh error: {}", error),
            Self::SshAuthFailure => write!(f, "failed to authenticate to remote server"),
            Self::SftpError(error) => write!(f, "sftp error: {}", error),
        }
    }
}

async fn get_sftp_client(app_config: &AppConfig) -> Result<SftpSession, SftpClientError> {
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
            return Err(SftpClientError::SshAuthFailure);
        }
    }

    let channel = session.channel_open_session().await?;
    channel.request_subsystem(true, "sftp").await?;
    Ok(SftpSession::new(channel.into_stream()).await?)
}
