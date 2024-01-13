use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use axum::extract::Extension;
use axum::response::IntoResponse;
use clap::Parser;
use log::*;
use serde::{Deserialize, Serialize};

use mpv_web_remote::Error as MpvError;
use mpv_web_remote::{Mpv, Request};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Failed to read screenshot")]
    Screenshot,
    #[error("Template not found")]
    TemplateNotFound,
    #[error(transparent)]
    Mpv(#[from] MpvError),
}

impl IntoResponse for Error {
    fn into_response(self) -> axum::response::Response {
        (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            self.to_string(),
        )
            .into_response()
    }
}

#[derive(clap::Parser)]
struct Flags {
    socket: PathBuf,
    #[clap(long, default_value = "0.0.0.0:3000")]
    addr: String,
    #[clap(long, short)]
    debug: bool,
    #[clap(flatten)]
    options: Options,
}

#[derive(clap::Parser, Clone)]
struct Options {
    /// Interval for backward seek [s]
    #[clap(long, default_value_t = 10)]
    rewind_offset_s: u16,
    /// Path to HTML template
    #[clap(long)]
    template: Option<PathBuf>,
}

fn setup_logging(debug: bool) -> anyhow::Result<()> {
    // Setup logging
    let colors = fern::colors::ColoredLevelConfig::new()
        .debug(fern::colors::Color::Blue)
        .info(fern::colors::Color::Green)
        .error(fern::colors::Color::Red)
        .warn(fern::colors::Color::Yellow);
    fern::Dispatch::new()
        .format(move |out, message, record| {
            out.finish(format_args!(
                "{} {} [{}] {}",
                chrono::Local::now().format("[%Y-%m-%d %H:%M:%S]"),
                colors.color(record.level()),
                record.target(),
                message,
            ))
        })
        .level(if debug {
            log::LevelFilter::Debug
        } else {
            log::LevelFilter::Info
        })
        .chain(std::io::stdout())
        .apply()?;
    Ok(())
}

async fn serve_binary(
    data: &'static [u8],
    content_type: &'static str,
) -> impl axum::response::IntoResponse {
    ([(axum::http::header::CONTENT_TYPE, content_type)], data)
}

async fn start(args: &Flags) -> anyhow::Result<()> {
    let options = Arc::new(args.options.clone());
    // Setup server
    info!("Connecting to server...");
    let (shutdown_rx, mpv) = loop {
        let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
        match Mpv::connect(&args.socket, || {
            let _ = shutdown_tx.send(());
        }) {
            Err(MpvError::Connection(e)) if e.kind() == std::io::ErrorKind::ConnectionRefused => {
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            }
            Err(e) => break Err(Error::Mpv(e)),
            Ok(mpv) => break Ok((shutdown_rx, mpv)),
        }
    }?;
    let mpv = Arc::new(Mutex::new(mpv));

    let app = axum::Router::new()
        .route("/", axum::routing::get(index))
        .route(
            "/jquery.min.js",
            axum::routing::get(|| async {
                serve_binary(include_bytes!("../jquery-3.7.0.min.js"), "text/javascript").await
            }),
        )
        .route(
            "/script.js",
            axum::routing::get(|| async {
                serve_binary(include_bytes!("../script.js"), "text/javascript").await
            }),
        )
        .route("/times", axum::routing::get(times))
        .route("/screenshot", axum::routing::get(serve_screenshot))
        .route("/action/:action", axum::routing::get(action))
        .layer(Extension(options))
        .layer(Extension(mpv));

    info!("Starting web server on http://{}", args.addr);
    axum::Server::bind(&args.addr.parse()?)
        .serve(app.into_make_service())
        .with_graceful_shutdown(async {
            shutdown_rx.await.ok();
            warn!("Connection closed, shutting down");
        })
        .await?;
    Ok(())
}

type MpvExt = Extension<Arc<Mutex<Mpv>>>;
async fn main_impl() -> anyhow::Result<()> {
    let args = Flags::parse();

    setup_logging(args.debug)?;

    if let Some(template) = &args.options.template {
        anyhow::ensure!(template.is_file(), "Template {:?} not found", template);
    }

    loop {
        if let Err(e) = start(&args).await {
            error!("{}", e);
        }
        warn!("Waiting 5 seconds until restarting");
        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
    }
}

fn format_duration(duration_s: f32) -> String {
    let duration_s = duration_s.round() as u32;
    let s = duration_s % 60;
    let min = (duration_s / 60) % 60;
    let h = (duration_s / 60) / 60;
    format!("{:0>2}:{:0>2}:{:0>2}", h, min, s)
}
#[derive(Serialize)]
struct Times {
    total: String,
    total_s: f32,
    current: String,
    current_s: f32,
    perc: f32,
}
async fn index(
    Extension(options): Extension<Arc<Options>>,
) -> Result<axum::response::Html<String>, Error> {
    let data = if let Some(template) = &options.template {
        std::fs::read_to_string(template).map_err(|_| Error::TemplateNotFound)?
    } else {
        include_str!("../templates/index.html").to_string()
    };
    Ok(axum::response::Html(data))
}
async fn times(Extension(mpv): MpvExt) -> Result<axum::response::Json<Times>, Error> {
    debug!("Handling times request");
    let mut mpv = mpv.lock().unwrap();
    let mut durations_s = vec![];
    for prop in ["playback-time", "duration"] {
        let duration_s: f32 = mpv.send(Request::get_property(prop))?.into_inner()?;
        durations_s.push(duration_s);
    }
    Ok(axum::Json(Times {
        current: format_duration(durations_s[0]),
        total: format_duration(durations_s[1]),
        total_s: durations_s[1],
        current_s: durations_s[0],
        perc: 100.0 * durations_s[0] / durations_s[1],
    }))
}
#[derive(Copy, Clone, Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
enum Action {
    Play,
    Pause,
    Rewind,
    Fullscreen,
    Seek,
}
#[derive(Deserialize)]
struct ActionQuery {
    position: Option<f32>,
}
/// Perform one of the actions
async fn action(
    axum::extract::Path(action): axum::extract::Path<Action>,
    axum::extract::Query(query): axum::extract::Query<ActionQuery>,
    Extension(mpv): MpvExt,
    Extension(options): Extension<Arc<Options>>,
) -> Result<axum::response::Response, Error> {
    info!("Handling {:?} request", action);
    let mut mpv = mpv.lock().unwrap();
    match action {
        Action::Play => {
            mpv.send(Request::set_property("pause", false))?;
        }
        Action::Pause => {
            mpv.send(Request::set_property("pause", true))?;
        }
        Action::Rewind => {
            mpv.send(Request::seek(-(options.rewind_offset_s as f32), "relative"))?;
            mpv.wait_event(|e| e.event == "playback-restart");
            mpv.send(Request::show_text(&format!(
                "Rewinded {} seconds",
                options.rewind_offset_s
            )))?;
        }
        Action::Fullscreen => {
            let current: bool = mpv
                .send(Request::get_property("fullscreen"))?
                .into_inner()?;
            mpv.send(Request::set_property("fullscreen", !current))?;
        }
        Action::Seek => {
            let Some(position) = query.position else {
                return Ok(axum::http::StatusCode::BAD_REQUEST.into_response());
            };
            mpv.send(Request::seek(position, "absolute-percent"))?;
            mpv.wait_event(|e| e.event == "playback-restart");
        }
    }
    Ok(axum::http::StatusCode::OK.into_response())
}
/// Return a downsampled JPG screenshot at the current location
async fn serve_screenshot(Extension(mpv): MpvExt) -> Result<axum::response::Response, Error> {
    debug!("Handling screenshot request");
    let mut mpv = mpv.lock().unwrap();
    let filename = "/tmp/mpv.jpg";
    mpv.send(Request::screenshot(filename))?;
    let image = image::io::Reader::open(filename)
        .map_err(|_| Error::Screenshot)?
        .decode()
        .map_err(|_| Error::Screenshot)?;
    let image = image.resize(300, 300, image::imageops::FilterType::Nearest);
    let data = vec![];
    let mut data = std::io::Cursor::new(data);
    image
        .write_to(&mut data, image::ImageOutputFormat::Jpeg(90))
        .map_err(|_| Error::Screenshot)?;
    Ok((
        [(axum::http::header::CONTENT_TYPE, "image/jpeg")],
        data.into_inner(),
    )
        .into_response())
}

#[tokio::main]
async fn main() {
    if let Err(e) = main_impl().await {
        error!("{:?}", e);
        std::process::exit(1);
    }
}
