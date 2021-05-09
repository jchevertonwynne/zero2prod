use std::net::TcpListener;

use sqlx::PgPool;
use zero2prod::configuration;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let config = configuration::get_config().expect("config file not found");
    let address = format!("127.0.0.1:{}", config.application_port);
    let listener = TcpListener::bind(address)?;
    let pg_pool = PgPool::connect(&config.database.connection_string())
        .await
        .expect("unable to create pool");
    zero2prod::startup::run_on(listener, pg_pool)?.await
}
