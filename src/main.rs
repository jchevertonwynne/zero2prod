use sqlx::PgPool;
use std::net::TcpListener;
use zero2prod::{
    configuration,
    telemetry::{get_subscriber, init_subscriber},
};

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let subscriber = get_subscriber("zero2prod".to_owned(), "info".to_owned(), std::io::stdout);
    init_subscriber(subscriber);

    let config = configuration::get_config().expect("config file not found");
    let address = format!("{}:{}", config.application.host, config.application.port);
    let listener = TcpListener::bind(address)?;
    let pg_pool = PgPool::connect(&config.database.connection_string())
        .await
        .expect(&format!("unable to create pool on address {}", config.database.connection_string()));
    zero2prod::startup::run_on(listener, pg_pool)?.await
}
