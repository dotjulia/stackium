use std::sync::{Arc, Mutex};

use rouille::{router, try_or_400, Response};

use crate::{
    debugger::{error::DebugError, Debugger},
    prompt::Command,
};

// static WEBSITE: &'static str = include_str!("../web/index.html");

fn index(debugger: Arc<Mutex<Debugger>>) -> rouille::Response {
    let debugger = debugger.lock().unwrap();
    rouille::Response::text(format!(
        "{} @ {}",
        debugger.program.to_str().unwrap(),
        debugger.child
    ))
}

fn ping() -> rouille::Response {
    rouille::Response::text("pong")
}

fn command(debugger: Arc<Mutex<Debugger>>, command: Command) -> rouille::Response {
    let mut debugger = debugger.lock().unwrap();
    let result = debugger.process_command(command);
    match result {
        Ok(output) => Response::json(&output),
        Err(err) => Response::text(format!("{:#?}", err)).with_status_code(500),
    }
}

pub fn start_webserver(debugger: Debugger) -> Result<(), DebugError> {
    let debugger = Arc::from(Mutex::new(debugger));
    println!("Now listening to localhost:8080");
    rouille::start_server("0.0.0.0:8080", move |request| {
        router!(request,
            (GET) (/) => {
                index(debugger.clone())
            },
            (GET) (/ping) => {
                ping()
            },
            (POST) (/command) => {
                let json = try_or_400!(rouille::input::json_input(request));
                command(debugger.clone(), json)
            },
            _ => rouille::Response::empty_404()
        )
    });
}
