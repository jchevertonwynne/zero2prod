use std::net::TcpListener;

use actix_web::{App, HttpResponse, HttpServer, Responder, dev::Server, web};

async fn health_check() -> impl Responder {
    HttpResponse::Ok()
}

pub fn run_default()  -> Result<Server, std::io::Error> {
    let listener = TcpListener::bind("127.0.0.1:8000")?;
    run(listener)
}

pub fn run(listener: TcpListener) -> Result<Server, std::io::Error> {
    let server = HttpServer::new(|| App::new().route("/health_check", web::get().to(health_check)))
        .listen(listener)?
        .run();

    Ok(server)
}