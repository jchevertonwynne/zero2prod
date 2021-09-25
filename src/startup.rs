use std::net::TcpListener;

use actix_web::{
    dev::Server,
    web::{get, post, Data},
    App, HttpServer,
};
use sqlx::{postgres::PgPoolOptions, PgPool};
use tracing_actix_web::TracingLogger;

use crate::{
    configuration::{DatabaseSettings, Settings},
    email_client::EmailClient,
    routes::{confirm_registration, health_check, publish_newsletter, subscribe},
};

pub struct Application {
    port: u16,
    server: Server,
}

impl Application {
    pub async fn build(config: Settings) -> Result<Self, std::io::Error> {
        let pg_pool = get_connection_pool(&config.database)
            .await
            .expect("failed to connect to postgres");

        let sender_email = config
            .email_client
            .sender()
            .expect("should be a valid email address");

        let email_client = EmailClient::new(
            config.email_client.base_url,
            sender_email,
            config.email_client.auth_token,
        );

        let address = format!("{}:{}", config.application.host, config.application.port);
        let listener = TcpListener::bind(address)?;
        let port = listener.local_addr().unwrap().port();

        let server = run_on(listener, pg_pool, email_client, config.application.base_url)?;

        Ok(Self { port, server })
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    pub async fn run_until_stopped(self) -> Result<(), std::io::Error> {
        self.server.await
    }
}

pub async fn get_connection_pool(config: &DatabaseSettings) -> Result<PgPool, sqlx::Error> {
    PgPoolOptions::new()
        .connect_timeout(std::time::Duration::from_secs(2))
        .connect_with(config.with_db())
        .await
}

pub struct ApplicationBaseUrl(pub String);

pub fn run_on(
    listener: TcpListener,
    pool: PgPool,
    email_client: EmailClient,
    base_url: String,
) -> Result<Server, std::io::Error> {
    let pool = Data::new(pool);
    let email_client = Data::new(email_client);
    let base_url = Data::new(ApplicationBaseUrl(base_url));
    let server = HttpServer::new(move || {
        App::new()
            .wrap(TracingLogger::default())
            .route("/health_check", get().to(health_check))
            .route("/subscriptions", post().to(subscribe))
            .route("/subscriptions/confirm", get().to(confirm_registration))
            .route("/newsletter", post().to(publish_newsletter))
            .app_data(Data::clone(&pool))
            .app_data(Data::clone(&email_client))
            .app_data(Data::clone(&base_url))
    })
    .listen(listener)?
    .run();

    Ok(server)
}
