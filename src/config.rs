use config::{Config, Environment};
use serde::Deserialize;

#[derive(Deserialize)]
pub struct SftpServerConfig {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: String,
    pub path: String,
}

#[derive(Deserialize)]
pub struct AppConfig {
    pub port: u16,
    pub sftp: SftpServerConfig,
}

pub fn get_app_config() -> AppConfig {
    let config = Config::builder()
        .add_source(Environment::default().separator("_"))
        .build()
        .unwrap();

    config.try_deserialize::<AppConfig>().unwrap()
}
