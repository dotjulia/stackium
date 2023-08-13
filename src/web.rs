use include_dir::{include_dir, Dir};
use stackium_shared::{Command, CommandOutput};
use tiny_http::{Header, Response, Server};

use crate::debugger::{error::DebugError, Debugger};

// static WEBSITE: &'static str = include_str!("../web/index.html");

static DIST_DIR: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/dist");

type ResponseType = Response<std::io::Cursor<Vec<u8>>>;

fn index(debugger: &mut Debugger) -> ResponseType {
    Response::from_string(format!(
        "{} @ {}",
        debugger.program.to_str().unwrap(),
        debugger.child
    ))
}

fn ping() -> ResponseType {
    Response::from_string("pong")
}

fn process_command(debugger: &mut Debugger, command: Command) -> ResponseType {
    let result = debugger.process_command(command);
    match result {
        Ok(output) => Response::from_string(serde_json::to_string(&output).unwrap())
            .with_header("Content-Type: application/json".parse::<Header>().unwrap()),
        Err(err) => Response::from_string(format!("{:#?}", err)).with_status_code(500),
    }
}

fn schema() -> ResponseType {
    Response::from_string(serde_json::to_string_pretty(&schemars::schema_for!(Command)).unwrap())
}

fn res_schema() -> ResponseType {
    Response::from_string(
        serde_json::to_string_pretty(&schemars::schema_for!(CommandOutput)).unwrap(),
    )
}

fn other(path: &str) -> ResponseType {
    let path = path.trim_start_matches("/");
    for file in DIST_DIR.files() {
        if file.path().file_name().unwrap() == path {
            return Response::from_data(file.contents()).with_header(format!("Content-Type: {}", mime_guess::from_path(path).first().unwrap_or(mime_guess::mime::TEXT_PLAIN)).parse::<Header>().unwrap());
        }
    }
    return Response::from_data([]).with_status_code(404);
}

pub fn start_webserver(mut debugger: Debugger) -> Result<(), DebugError> {
    println!("API available at localhost:8080");
    let server = Server::http("0.0.0.0:8080").unwrap();
    println!("UI available at http://localhost:8080/index.html");
    for mut request in server.incoming_requests() {
        match request.method() {
            tiny_http::Method::Get => match request.url() {
                "/schema" => request.respond(schema()),
                "/response_schema" => request.respond(res_schema()),
                "/" => request.respond(index(&mut debugger)),
                "/ping" => request.respond(ping()),
                path => {
                    let path = path.to_string();
                    request.respond(other(&path))
                },
            },
            tiny_http::Method::Post => match request.url() {
                "/command" => {
                    let mut content = String::new();
                    request.as_reader().read_to_string(&mut content).unwrap();
                    let command = serde_json::from_str(&content);
                    match command {
                        Ok(command) => request.respond(process_command(&mut debugger, command)),
                        Err(e) => request.respond(
                            Response::from_string(format!("{:#?}", e)).with_status_code(500),
                        ),
                    }
                }
                _ => request.respond(Response::empty(404)),
            },
            _ => request.respond(Response::empty(404)),
        }
        .unwrap_or_else(|e| eprintln!("Failed to respond to request {}", e));
    }
    Ok(())
}
