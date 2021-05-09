use std::net::TcpListener;

use configuration::DatabaseSettings;
use sqlx::{Connection, Executor, PgConnection, PgPool};
use uuid::Uuid;
use zero2prod::configuration;

pub struct TestApp {
    pub address: String,
    pub db_pool: PgPool
}

#[actix_rt::test]
async fn health_check_works() {
    let test_app = spawn_app().await;
    let client = reqwest::Client::new();
    let response = client
        .get(format!("{}/health_check", test_app.address))
        .send()
        .await
        .expect("failed to execute request");

    assert_eq!(response.status().as_u16(), 200);
    assert_eq!(response.content_length(), Some(0));
}

#[actix_rt::test]
async fn subscribe_returns_200_for_valid_form() {
    let test_app = spawn_app().await;
    let client = reqwest::Client::new();
    let body = "name=joseph&email=jchevertonwynne%40gmail.com";

    let response = client
        .post(format!("{}/subscriptions", test_app.address))
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body(body)
        .send()
        .await
        .expect("failed to execute request");

    assert_eq!(response.status().as_u16(), 200);
    let saved = sqlx::query!("select email, name from subscriptions")
        .fetch_one(&test_app.db_pool)
        .await
        .expect("failed to execute query");
    assert_eq!(saved.email, "jchevertonwynne@gmail.com");
    assert_eq!(saved.name, "joseph");
}

#[actix_rt::test]
async fn subscribe_returns_400_for_invalid_form() {
    let test_app = spawn_app().await;
    let client = reqwest::Client::new();

    let test_cases = [
        ("name=joseph", "missing email"),
        ("email=jchevertonwynne%40gmail.com", "missing name"),
        ("", "missing both params"),
    ];

    for &(body, err_msg) in test_cases.iter() {
        let response = client
            .post(format!("{}/subscriptions", test_app.address))
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(body)
            .send()
            .await
            .expect("failed to execute request");

        assert_eq!(
            response.status().as_u16(),
            400,
            "the api did not return a 400 when {}",
            err_msg
        );
    }
}

async fn spawn_app() -> TestApp {
    let listener = TcpListener::bind("localhost:0").expect("failed to find random port");
    let port = listener.local_addr().expect("should have port").port();

    let mut config = configuration::get_config().expect("failed to load config");
    config.database.database_name = Uuid::new_v4().to_string();
    let pg_pool = configure_database(&config.database).await;

    let server = zero2prod::startup::run_on(listener, pg_pool.clone())
        .expect("failed to bind address");
    tokio::spawn(server);
    let address = format!("http://localhost:{}", port);

    TestApp {
        address,
        db_pool: pg_pool
    }
}

async fn configure_database(config: &DatabaseSettings) -> PgPool {
    let mut connection = PgConnection::connect(&config.connection_string_without_db()).await.expect("failed to connect to postgres");
    connection.execute(&*format!(r#"CREATE DATABASE "{}";"#, &config.database_name)).await.expect("failed to create database");

    let connection_pool = PgPool::connect(&config.connection_string()).await.expect("failed to connect to postgres db");
    sqlx::migrate!("./migrations").run(&connection_pool).await.expect("failed to migrate the db");
    connection_pool
}