#[actix_web::main]
async fn main() -> std::io::Result<()> {
    zero2prod::run_default()?.await
}
