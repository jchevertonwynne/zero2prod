use once_cell::sync::Lazy;
use reqwest::Url;
use sha3::Digest;
use sqlx::{Connection, Executor, PgConnection, PgPool};
use uuid::Uuid;
use wiremock::MockServer;
use zero2prod::{
    configuration::{get_config, DatabaseSettings},
    startup::{get_connection_pool, Application},
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
    pub email_server: MockServer,
    pub port: u16,
    pub test_user: TestUser
}

pub struct ConfirmationLinks {
    pub html: reqwest::Url,
    pub plain: reqwest::Url,
}

impl TestApp {
    pub async fn post_subscriptions(&self, body: String) -> reqwest::Response {
        reqwest::Client::new()
            .post(format!("{}/subscriptions", self.address))
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(body)
            .send()
            .await
            .expect("failed to execute request")
    }

    pub async fn post_newsletter(&self, body: serde_json::Value) -> reqwest::Response {
        reqwest::Client::new()
            .post(&format!("{}/newsletter", &self.address))
            .basic_auth(&self.test_user.username, Some(&self.test_user.password))
            .json(&body)
            .send()
            .await
            .expect("failed to execute request")
    }

    pub fn get_confirmation_links(&self, email_request: &wiremock::Request) -> ConfirmationLinks {
        let body: serde_json::Value = serde_json::from_slice(&email_request.body).unwrap();

        let get_link = |s: &str| {
            let found = linkify::LinkFinder::new()
                .links(s)
                .filter(|l| *l.kind() == linkify::LinkKind::Url)
                .next()
                .unwrap()
                .as_str()
                .to_owned();
            let mut url = Url::parse(&found).unwrap();
            url.set_port(Some(self.port)).unwrap();
            url
        };

        let html = get_link(&body["HtmlBody"].as_str().unwrap());
        let plain = get_link(&body["TextBody"].as_str().unwrap());
        ConfirmationLinks { html, plain }
    }
}

pub async fn spawn_app() -> TestApp {
    Lazy::force(&TRACING);

    let email_server = MockServer::start().await;

    let config = {
        let mut c = get_config().expect("failed to read config");
        c.database.database_name = Uuid::new_v4().to_string();
        c.application.port = 0;
        c.email_client.base_url = email_server.uri();
        c
    };

    configure_database(&config.database).await;

    let application = Application::build(config.clone())
        .await
        .expect("failed to build application");
    let port = application.port();
    let address = format!("http://127.0.0.1:{}", application.port());

    let db_pool = get_connection_pool(&config.database)
        .await
        .expect("failed to connect to db");

    let test_user = TestUser::generate();

    test_user.store(&db_pool).await;

    TestApp {
        address,
        db_pool,
        email_server,
        port,
        test_user
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

pub struct TestUser {
    pub user_id: Uuid,
    pub username: String,
    pub password: String
}

impl TestUser {
    pub fn generate() -> Self {
        Self {
            user_id: Uuid::new_v4(), 
            username: Uuid::new_v4().to_string(), 
            password: Uuid::new_v4().to_string()
        }
    }

    async fn store(&self, pool: &PgPool) {
        let digest = sha3::Sha3_256::digest(self.password.as_bytes());
        let password_hash = format!("{:x}", digest);
        sqlx::query!(
            "INSERT INTO users (user_id, username, password_hash)
            VALUES ($1, $2, $3)", 
            self.user_id,
            self.username, 
            password_hash
        ).execute(pool).await.expect("failed to create test users");
    }
}
