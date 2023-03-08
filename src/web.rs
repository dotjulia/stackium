use std::{fs::File, str::FromStr};

use addr2line::gimli::Reader;
use tiny_http::{Response, Server};

use crate::{debugger::Debugger, prompt::Command};

static WEBSITE: &'static str = include_str!("../web/index.html");

pub fn serve_web<R: Reader + PartialEq>(mut debugger: Debugger<R>) {
    let server = Server::http("0.0.0.0:5146").unwrap();
    println!("Listening on http://0.0.0.0:5146");
    for mut request in server.incoming_requests() {
        match request.method() {
            tiny_http::Method::Get => match request.url() {
                "/" => request.respond(Response::from_file(File::open("web/index.html").unwrap())),
                "/pid" => request.respond(Response::from_string(format!("{}", debugger.child))),
                _ => request.respond(Response::empty(404)),
            },
            tiny_http::Method::Post => match request.url() {
                "/command" => {
                    let mut content = String::new();
                    request.as_reader().read_to_string(&mut content).unwrap();
                    if let Ok(cmd) = Command::from_str(&content) {
                        let response = debugger.process_command(cmd);
                        if let Ok(response) = response {
                            request.respond(Response::from_string(
                                serde_json::to_string(&response).unwrap(),
                            ))
                        } else {
                            request.respond(Response::from_string(
                                "{\"error\": \"Failed to execute command\"}".to_string(),
                            ))
                        }
                    } else {
                        request.respond(Response::from_string(
                            "{\"error\": \"Couldn't parse command\"}".to_string(),
                        ))
                    }
                }
                _ => request.respond(Response::empty(404)),
            },
            _ => request.respond(Response::empty(404)),
        }
        .unwrap_or_else(|e| eprintln!("Error while responding to request: {}", e));
    }
}
