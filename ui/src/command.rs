use ehttp::{fetch, Request};
use poll_promise::Promise;
use stackium_shared::{Command, CommandOutput};
use url::Url;

pub fn dispatchCommand(
    backend_url: Url,
    command: Command,
) -> Promise<Result<CommandOutput, String>> {
    let (sender, promise) = Promise::new();
    let request = Request::post(
        backend_url.join("/command").unwrap(),
        serde_json::to_vec(&command).unwrap(),
    );
    fetch(request, move |response| match response {
        Ok(response) => {
            let body = response.text();
            match body {
                Some(body) => {
                    let output: CommandOutput = serde_json::from_str(&body).unwrap();
                    sender.send(Ok(output));
                }
                None => sender.send(Err("Failed to parse response".to_string())),
            }
        }
        Err(e) => sender.send(Err(format!("Error: {}", e))),
    });
    promise
}
