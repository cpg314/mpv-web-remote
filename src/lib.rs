//! This crate contains
//! - A client library for [mpv's JSON remote control interface over unix sockets](https://mpv.io/manual/stable/#json-ipc).
//! - A web server providing a remote control user interface (binary target `mpv-web-remote`).
//!
//! # Client library
//! After launching mpv with `--input-ipc-server=/tmp/mpv`:
//! ```
//! let mut client = Mpv::connect("/tmp/mpv", || {})?;
//! client.send(Request::show_text("Test"))?;
//! let duration_s: f32 = client.send(Request::get_property(prop))?.into_inner()?;
//! ```
//! See the other available requests in [`Request`].
//! # Remote control web server
//! See the README.
//! This requires the `server` feature (enabled by default).
mod ipc;
pub use ipc::*;
mod messages;
pub use messages::Messages;

/// Main error type
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Missing expected data field in response")]
    MissingData,
    #[error("Connection closed")]
    StreamClosed,
    #[error("Failed to read from stream: {0}")]
    Read(std::io::Error),
    #[error("Failed to downcast data: {0}")]
    Downcasting(serde_json::Error),
    #[error("Failed to deserialize response from server: {0}")]
    JsonDeser(serde_json::Error),
    #[error("Server returned an error: {0}")]
    ServerError(String),
    #[error("Failed to serialize request: {0}")]
    JsonSer(serde_json::Error),
    #[error("Failed to write to socket: {0}")]
    Write(std::io::Error),
    #[error("Failed to connect to socket: {0}")]
    Connection(std::io::Error),
}
