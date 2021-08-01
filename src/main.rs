use zero2prod::{
    configuration,
    startup::Application,
    telemetry::{get_subscriber, init_subscriber},
};

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let subscriber = get_subscriber("zero2prod".to_owned(), "info".to_owned(), std::io::stdout);
    init_subscriber(subscriber);

    let config = configuration::get_config().expect("config file not found");
    let application = Application::build(config).await?;
    application.run_until_stopped().await?;
    Ok(())
}
