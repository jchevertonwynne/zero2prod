use std::net::TcpListener;

use once_cell::sync::Lazy;
use sqlx::{Connection, Executor, PgConnection, PgPool};
use uuid::Uuid;
use zero2prod::{
    configuration::{self, DatabaseSettings},
    telemetry::{get_subscriber, init_subscriber},
};

static TRACING: Lazy<()> = Lazy::new(|| {
    let default_filter_level = "info".to_owned();
    let subscriber_name = "test".to_owned();

    if std::env::var("TEST_LOG").is_ok() {
        let subscriber = get_subscriber(subscriber_name, default_filter_level, std::io::stdout);
        init_subscriber(subscriber);
    } else {
        let subscriber = get_subscriber(subscriber_name, default_filter_level, std::io::sink);
        init_subscriber(subscriber);
    }
});

pub struct TestApp {
    pub address: String,
    pub db_pool: PgPool,
}

pub async fn spawn_app() -> TestApp {
    Lazy::force(&TRACING);

    let listener = TcpListener::bind("localhost:0").expect("failed to find random port");
    let port = listener.local_addr().expect("should have port").port();
    let address = format!("http://localhost:{}", port);

    let mut config = configuration::get_config().expect("failed to load config");
    config.database.database_name = Uuid::new_v4().to_string();
    let pg_pool = configure_database(&config.database).await;

    let server =
        zero2prod::startup::run_on(listener, pg_pool.clone()).expect("failed to bind address");
    tokio::spawn(server);

    TestApp {
        address,
        db_pool: pg_pool,
    }
}

async fn configure_database(config: &DatabaseSettings) -> PgPool {
    let mut connection = PgConnection::connect_with(&config.without_db())
        .await
        .expect("failed to connect to postgres");
    connection
        .execute(&*format!(r#"CREATE DATABASE "{}";"#, &config.database_name))
        .await
        .expect("failed to create database");

    let connection_pool = PgPool::connect_with(config.with_db())
        .await
        .expect("failed to connect to postgres db");
    sqlx::migrate!("./migrations")
        .run(&connection_pool)
        .await
        .expect("failed to migrate the db");
    connection_pool
}
