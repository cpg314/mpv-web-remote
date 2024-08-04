# mpv-web-remote

Simple zero-dependency web remote for the [mpv media player](https://mpv.io/), using the [JSON IPC interface](https://mpv.io/manual/stable/#json-ipc).

![Screenshot](screenshot.png)

A single binary on the host machine connects to the mpv [unix socket](https://en.wikipedia.org/wiki/Unix_domain_socket) and exposes a remote control via a web server, which can be accessed from any device on the same network (e.g. a smartphone). The web app announces itself via the [media session API](https://developer.mozilla.org/en-US/docs/Web/API/Media_Session_API), allowing control even when the app is in the background, or from connected devices (e.g. a smartwatch).

![Screenshot](mediasession.png)

The client code can also be used separately to interface with mpv from Rust in other use cases.

## Features

For now, only the following basic functions are supported:

- Play/pause
- Toggle full-screen
- Rewind 10 seconds (can be changed with the `--rewind-s` option)
- Visualize progress (current time, total time, percentage)
- Preview (updated every 3 seconds)
- Seek (by clicking on the progress bar)

## Installation

First, edit your `~/.config/mpv/mpv.conf` to add

```
input-ipc-server=~/mpv
```

The `.deb` and Arch Linux/Manjaro packages in the [releases page](https://github.com/cpg314/mpv-web-remote/releases) will install `mpv-web-remote` as well as a `systemd` service.

It should suffice to enable it with

```console
$ systemctl --user enable --now mpv-web-remote
$ systemctl --user status mpv-web-remote
INFO [mpv_rs::mpv] Connected to socket
INFO [mpv_rs] Starting web server on http://0.0.0.0:3000
```

The web server will then be enabled shortly after a new instance of mpv is started and binds to the socket.

With the default parameters, the control interface will be accessible on <http://[ip]:3000>.

> [!WARNING]  
> There is currently no authentication.

### Alternative: Manual installation

```console
$ cp mpv-web-remote /usr/local/bin/
$ cp mpv-web-remote.service ~/.config/systemd/user/
$ systemctl --user daemon-reload && systemctl --user enable --now mpv-web-remote
```

### Alternative: Without systemd

The server can also be started manually:

```console
$ mpv-web-remote --help
Usage: mpv-web-remote [OPTIONS] <SOCKET>

Arguments:
  <SOCKET>

Options:
      --addr <ADDR>                        [default: 0.0.0.0:3000]
  -d, --debug
      --rewind-offset-s <REWIND_OFFSET_S>  Interval for backward seek [s] [default: 10]
      --template <TEMPLATE>                Path to HTML template
  -h, --help                               Print help
```

## Implementation details

There are two components:

- the interface with mpv
- the web server

The web server is straightforward and is implemented with [axum](https://docs.rs/axum/latest/axum/). For page interactions, we simply use [jQuery](https://jquery.com/).

The mpv interface takes the form

```rust
impl Mpv {
    pub fn connect(socket: impl AsRef<Path>) -> Result<Self, Error>;
    pub fn send(&mut self, mut request: Request) -> Result<Response, Error>;
    pub fn wait_event(&self, filter: impl Fn(&Event) -> bool);
}
```

The `connect` constructor connects to the socket and creates a thread responsible for reading replies and events from the servers into a buffer. In particular, we are robust against changes to this implementation detail:

> "Currently, the mpv-side IPC implementation does not service the socket while a command is executed and the reply is written. It is for example not possible that other events, that happened during the execution of the command, are written to the socket before the reply is written.
>
> This might change in the future. The only guarantee is that replies to IPC messages are sent in sequence."

The `send` method writes a request to the socket, serialized using [`serde_json`](https://docs.rs/serde_json/latest/serde_json/). It then blocks until the server responds to that request.

A selection of [commands](https://mpv.io/manual/stable/#list-of-input-commands) is implemented:

```rust
impl Request {
    pub fn playback_time() -> Self;
    pub fn get_property(property: &str) -> Self;
    pub fn seek(target: f32, flags: &str) -> Self;
    pub fn set_property<T: Into<serde_json::Value>>(property: &str, value: T) -> Self;
    pub fn show_text(text: &str) -> Self;
    pub fn observe_property(id: i64, property: &str) -> Self;
    pub fn screenshot<P: AsRef<Path> + ?Sized>(filename: &P) -> Self;
}
```

A generic response takes the form

```rust
#[derive(Deserialize, Debug)]
pub struct Response {
    pub request_id: i64,
    pub data: Option<serde_json::Value>,
    pub error: String,
}
```

and can be downcast to the expected data type

```rust
impl Response {
    pub fn into_inner<T: DeserializeOwned>(self) -> Result<T, Error>;
}
```

This could also be encapsulated into higher-level methods (e.g. `get_playback_time() -> f64`).

Finally, the `wait_event` method simply blocks until an event occurs. This is for example useful to only trigger a screenshot at the new position after a seek has finished.

## Existing solutions

A quick search will reveal two "mpv remote" Android apps using the aforementioned mpv control interface. Naturally, they also require an additional component to run on the host machine to expose the API to the app.

- <https://github.com/husudosu/mpv-remote-app>, where a Node.JS server (<https://github.com/husudosu/mpv-remote-node>) runs as an mpv plugin and provides an API over HTTP.
- <https://github.com/mcastorina/mpv-remote-app>, where a separate Python server exposes an API over a network socket.

The [mpvipc](https://crates.io/crates/mpvipc) crate provides a similar and more complete interface, but the above was an opportunity to experiment with a different implementation, under a more permissive license.
