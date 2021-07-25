use serde_aux::field_attributes::deserialize_number_from_string;
use sqlx::postgres::{PgConnectOptions, PgSslMode};
use std::convert::{TryFrom, TryInto};

#[derive(serde::Deserialize)]
pub struct Settings {
    pub database: DatabaseSettings,
    pub application: ApplicationSettings,
}

#[derive(serde::Deserialize)]
pub struct ApplicationSettings {
    #[serde(deserialize_with = "deserialize_number_from_string")]
    pub port: u16,
    pub host: String,
}

#[derive(serde::Deserialize)]
pub struct DatabaseSettings {
    pub username: String,
    pub password: String,
    #[serde(deserialize_with = "deserialize_number_from_string")]
    pub port: u16,
    pub host: String,
    pub database_name: String,
    pub require_ssl: bool,
}

pub fn get_config() -> Result<Settings, config::ConfigError> {
    let mut settings = config::Config::default();
    let base_path = std::env::current_dir().expect("failed to get current directory");
    let configuration_directory = base_path.join("configuration");

    settings.merge(config::File::from(configuration_directory.join("base")).required(true))?;

    let environment: Environment = std::env::var("APP_ENVIRONMENT")
        .unwrap_or_else(|_| "local".into())
        .try_into()
        .expect("failed to parse APP_ENVIRONMENT");

    settings.merge(
        config::File::from(configuration_directory.join(environment.to_string().as_str()))
            .required(true),
    )?;
    settings.merge(config::Environment::with_prefix("app").separator("__"))?;

    settings.try_into()
}

enum Environment {
    Local,
    Production,
}

impl ToString for Environment {
    fn to_string(&self) -> String {
        match self {
            Environment::Local => "local".into(),
            Environment::Production => "production".into(),
        }
    }
}

impl TryFrom<String> for Environment {
    type Error = String;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        match value.to_lowercase().as_str() {
            "local" => Ok(Self::Local),
            "production" => Ok(Self::Production),
            _ => Err(format!(
                "{} is not a supported environment, use `local` or `production`",
                value
            )),
        }
    }
}

impl DatabaseSettings {
    pub fn without_db(&self) -> PgConnectOptions {
        let ssl_mode = if self.require_ssl {
            PgSslMode::Require
        } else {
            PgSslMode::Prefer
        };

        PgConnectOptions::new()
            .host(&self.host)
            .username(&self.username)
            .password(&self.password)
            .port(self.port)
            .ssl_mode(ssl_mode)
    }

    pub fn with_db(&self) -> PgConnectOptions {
        self.without_db().database(&self.database_name)
    }
}
