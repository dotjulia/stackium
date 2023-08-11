use ehttp::{fetch, Request};
use poll_promise::Promise;
use stackium_shared::{Command, CommandOutput};
use url::Url;

macro_rules! dispatch {
    ($url:expr, $command:expr, $out:ident) => {
        crate::command::dispatch_command_and_then($url, $command, |out| match out {
            CommandOutput::$out(a) => a,
            _ => unreachable!(),
        })
    };
}

pub(crate) use dispatch;

pub fn dispatch_command_and_then<T: Send>(
    backend_url: Url,
    command: Command,
    and_then: impl FnOnce(CommandOutput) -> T + Send + 'static,
) -> Promise<Result<T, String>> {
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
                    let output = and_then(output);
                    sender.send(Ok(output));
                }
                None => sender.send(Err("Failed to parse response".to_string())),
            }
        }
        Err(e) => sender.send(Err(format!("Error: {}", e))),
    });
    promise
}
