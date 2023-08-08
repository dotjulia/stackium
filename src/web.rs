use std::sync::Mutex;

use crate::{debugger::Debugger, prompt::Command};
use actix_web::{
    get, post,
    web::{Data, Json},
    App, HttpResponse, HttpServer, Responder,
};

// static WEBSITE: &'static str = include_str!("../web/index.html");

struct AppState {
    debugger: Mutex<Debugger>,
}

#[get("/")]
async fn index(data: Data<AppState>) -> impl Responder {
    let debugger = data.debugger.lock().unwrap();
    HttpResponse::Ok().body(format!(
        "{} @ {}",
        debugger.program.to_str().unwrap(),
        debugger.child
    ))
}

#[get("/ping")]
async fn ping() -> impl Responder {
    HttpResponse::Ok().body("pong")
}

#[post("/command")]
async fn command(data: Data<AppState>, command: Json<Command>) -> impl Responder {
    let mut debugger = data.debugger.lock().unwrap();
    let command = command.0;
    let result = debugger.process_command(command);
    match result {
        Ok(output) => HttpResponse::Ok().json(output),
        Err(err) => HttpResponse::InternalServerError().body(format!("{:#?}", err)),
    }
}

pub async fn start_webserver(debugger: Debugger) -> std::io::Result<()> {
    std::env::set_var("RUST_LOG", "debug");
    env_logger::init();
    let debugger = Mutex::new(debugger);
    let data = Data::new(AppState { debugger });
    HttpServer::new(move || {
        App::new()
            .app_data(data.clone())
            .service(index)
            .service(ping)
            .service(command)
    })
    .bind(("127.0.0.1", 8080))?
    .run()
    .await
}
