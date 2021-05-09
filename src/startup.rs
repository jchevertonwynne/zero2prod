use std::net::TcpListener;

use actix_web::{dev::Server, web, App, HttpServer};

use crate::routes::{health_check, subscribe};

pub fn run() -> Result<Server, std::io::Error> {
    let listener = TcpListener::bind("127.0.0.1:8000")?;
    run_on(listener)
}

pub fn run_on(listener: TcpListener) -> Result<Server, std::io::Error> {
    let server = HttpServer::new(|| {
        App::new()
            .route("/health_check", web::get().to(health_check))
            .route("/subscriptions", web::post().to(subscribe))
    })
    .listen(listener)?
    .run();

    Ok(server)
}
