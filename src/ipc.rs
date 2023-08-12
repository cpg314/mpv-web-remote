//! Client for the mpv remote control interface

use std::io::prelude::*;
use std::io::BufReader;
use std::os::unix::net::UnixStream;
use std::path::Path;

use log::*;
use serde::{de::DeserializeOwned, Deserialize, Serialize};

use crate::Error;
use crate::Messages;

/// Request to send to mpv
#[derive(Debug, Serialize)]
pub struct Request {
    command: Vec<serde_json::Value>,
    request_id: i64,
}
/// Response to a request by mpv
#[derive(Deserialize, Debug)]
pub struct Response {
    pub request_id: i64,
    pub data: Option<serde_json::Value>,
    pub error: String,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum ServerMessage {
    Response(Response),
    Event(Event),
}
/// Event forwarded by mpv
#[derive(Debug, Deserialize)]
pub struct Event {
    pub event: String,
    pub id: Option<i64>,
    pub name: Option<String>,
    pub data: Option<serde_json::Value>,
}
impl Response {
    /// Ensure that the response is not an error.
    pub fn check_error(&self) -> Result<(), Error> {
        if self.error != "success" {
            return Err(Error::ServerError(self.error.clone()));
        }
        Ok(())
    }
    /// Downcast the response data to a given type.
    ///
    /// This will call [`Self::check_error`].
    pub fn into_inner<T: DeserializeOwned>(self) -> Result<T, Error> {
        self.check_error()?;
        let data = self.data.ok_or(Error::MissingData)?;
        serde_json::from_value(data).map_err(Error::Downcasting)
    }
}
impl Request {
    /// Equivalent to [`Self::get_property`] for the `playback-time` property.
    pub fn playback_time() -> Self {
        Self::get_property("playback-time")
    }
    /// Retrieve the value of a property. See <https://mpv.io/manual/stable/#properties>
    ///
    /// Use [`Response::into_inner`] to downcase the response into the expected type.
    pub fn get_property(property: &str) -> Self {
        Self {
            command: vec!["get_property".into(), property.into()],
            request_id: 0,
        }
    }
    /// Seek at a given location. See <https://mpv.io/manual/stable/#list-of-input-commands>.
    pub fn seek(target: f32, flags: &str) -> Self {
        Self {
            command: vec!["seek".into(), target.into(), flags.into()],
            request_id: 0,
        }
    }
    /// Set the value of a property. See <https://mpv.io/manual/stable/#properties>
    pub fn set_property<T: Into<serde_json::Value>>(property: &str, value: T) -> Self {
        Self {
            command: vec!["set_property".into(), property.into(), value.into()],
            request_id: 0,
        }
    }
    /// Display text on the screeen. See <https://mpv.io/manual/stable/#list-of-input-commands>.
    pub fn show_text(text: &str) -> Self {
        Self {
            command: vec!["show-text".into(), text.into()],
            request_id: 0,
        }
    }
    /// Subscribe to events. See <https://mpv.io/manual/stable/#properties>
    pub fn observe_property(id: i64, property: &str) -> Self {
        Self {
            command: vec!["observe_property".into(), id.into(), property.into()],
            request_id: 0,
        }
    }
    /// Trigger a screenshot. See <https://mpv.io/manual/stable/#list-of-input-commands>
    pub fn screenshot<P: AsRef<Path> + ?Sized>(filename: &P) -> Self {
        Self {
            command: vec![
                "screenshot-to-file".into(),
                filename.as_ref().to_str().unwrap().into(),
            ],
            request_id: 0,
        }
    }
}
/// mpv remote control client
pub struct Mpv {
    stream: UnixStream,
    messages: Messages<ServerMessage>,
    _reader: std::thread::JoinHandle<Result<(), Error>>,
    request_id: i64,
}
impl Mpv {
    /// Connect to a socket.
    ///
    /// `on_shutdown` is called when the message processing thread receives an EOF from the socket.
    pub fn connect(
        socket: impl AsRef<Path>,
        on_shutdown: impl FnOnce() + Send + 'static,
    ) -> Result<Self, Error> {
        debug!("Connecting to socket");
        let stream = UnixStream::connect(socket).map_err(Error::Connection)?;
        info!("Connected to socket");
        let mut reader = BufReader::new(stream.try_clone().map_err(Error::Connection)?);
        let messages = Messages::default();
        let messages2 = messages.clone();
        let reader = std::thread::spawn(move || {
            let mut line = String::default();
            let mut process_one = move || -> Result<(), Error> {
                line.clear();
                if reader.read_line(&mut line).map_err(Error::Read)? == 0 {
                    return Err(Error::StreamClosed);
                }
                debug!("Read line {:?} from server", line);
                let resp: ServerMessage = serde_json::from_str(&line).map_err(Error::JsonDeser)?;
                debug!("Parsed response {:?} from server", resp);
                messages2.push(resp);
                Ok(())
            };
            loop {
                if let Err(e) = process_one() {
                    if let Error::StreamClosed = e {
                        warn!("Socket closed, shutting down");
                        on_shutdown();

                        break;
                    } else {
                        error!("Error processing server message: {}", e);
                    }
                }
            }
            Ok(())
        });
        info!("Connected to socket");
        Ok(Self {
            stream,
            _reader: reader,
            messages,
            request_id: 0,
        })
    }
    /// Send a request, wait for the response, and check whether it was successful.
    ///
    /// ### Warning
    /// This will currently block indefinitely if the server does not return a response.
    pub fn send(&mut self, mut request: Request) -> Result<Response, Error> {
        self.request_id += 1;
        request.request_id = self.request_id;
        debug!("Sending requet {:?}", request);
        serde_json::to_writer(&mut self.stream, &request).map_err(Error::JsonSer)?;
        self.stream.write(b"\n").map_err(Error::Write)?;
        let m = self.messages.wait(
            |s| matches!(s, ServerMessage::Response(r) if r.request_id == request.request_id),
        );
        if let ServerMessage::Response(r) = m {
            r.check_error()?;
            Ok(r)
        } else {
            unreachable!()
        }
    }
    /// Wait for an event to occur.
    pub fn wait_event(&self, filter: impl Fn(&Event) -> bool) {
        self.messages
            .wait(|s| matches!(s, ServerMessage::Event(e) if filter(e)));
    }
}
