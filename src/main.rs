mod printmode;
mod robot;
mod scaling;
mod timelimits;

use std::convert::From;
use std::ffi::OsStr;
use std::fmt;
use std::fs::{read_dir, DirEntry, File};
use std::io::{self, Read, Write};
use std::path::Path;
use std::process;
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};
use std::thread::sleep;
use std::time::Duration;

use actix_web::http::StatusCode;
use actix_web::HttpServer;
use actix_web::{get, post, web, App, HttpResponse, Responder, ResponseError};
use docopt::Docopt;
use log::{error, info};
use rust_embed::RustEmbed;
use serde_derive::{Deserialize, Serialize};
use serial::BaudRate;
use simplelog::{
    ColorChoice, Config as LogConfig, LevelFilter, SimpleLogger, TermLogger, TerminalMode,
};
use svg2polylines::Polyline;

use crate::printmode::PrintMode;
use crate::robot::PrintTask;
use crate::scaling::{Bounds, Range};
use crate::timelimits::TimeLimits;

type RobotQueue = Arc<Mutex<Sender<PrintTask>>>;

// Suggested value from https://docs.rs/svg2polylines/0.7.0/svg2polylines/fn.parse.html
const SVG2POLYLINES_TOLERANCE: f64 = 0.15;

/// The raw configuration obtained when parsing the config file.
#[derive(Debug, Deserialize, Clone)]
struct RawConfig {
    listen: Option<String>,
    device: Option<String>,
    svg_dir: Option<String>,
    interval_seconds: Option<u64>,
    time_limits: Option<TimeLimits>,
}

/// Note: This struct can be queried over HTTP,
/// so be careful with sensitive data.
#[derive(Debug, Serialize, Clone)]
struct Config {
    listen: String,
    device: String,
    svg_dir: String,
    interval_seconds: u64,
    time_limits: Option<TimeLimits>,
}

impl Config {
    fn from(config: &RawConfig) -> Option<Self> {
        let listen = match config.listen {
            Some(ref val) => val.clone(),
            None => "127.0.0.1:8080".to_string(),
        };
        let device = match config.device {
            Some(ref val) => val.clone(),
            None => {
                info!("Note: Config is missing device key");
                return None;
            }
        };
        let svg_dir = match config.svg_dir {
            Some(ref val) => val.clone(),
            None => {
                info!("Note: Config is missing svg_dir key");
                return None;
            }
        };
        let interval_seconds = match config.interval_seconds {
            Some(val) => val,
            None => {
                info!("Note: Config is missing interval_seconds key");
                return None;
            }
        };
        let time_limits = config.time_limits;
        Some(Self {
            listen,
            device,
            svg_dir,
            interval_seconds,
            time_limits,
        })
    }
}

#[derive(Debug, Clone)]
struct PreviewConfig {
    listen: String,
}

impl PreviewConfig {
    fn from(config: &RawConfig) -> Self {
        Self {
            listen: config
                .listen
                .clone()
                .unwrap_or_else(|| "listen".to_string()),
        }
    }
}

/// Application state.
/// Every worker will have its own copy.
#[derive(Debug, Clone)]
struct State {
    config: Config,
    robot_queue: RobotQueue,
}

#[derive(Debug)]
enum HeadlessError {
    NoFiles,
    Io(io::Error),
    SvgParse(String),
    PolylineScale(String),
    Queue(String),
}

impl From<io::Error> for HeadlessError {
    fn from(e: io::Error) -> Self {
        HeadlessError::Io(e)
    }
}

impl fmt::Display for HeadlessError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            HeadlessError::NoFiles => write!(f, "No SVG files found"),
            HeadlessError::Io(e) => write!(f, "I/O Error: {}", e),
            HeadlessError::SvgParse(e) => write!(f, "SVG Parse Error: {}", e),
            HeadlessError::PolylineScale(e) => write!(f, "Polyline Scaling Error: {}", e),
            HeadlessError::Queue(e) => write!(f, "Queue Error: {}", e),
        }
    }
}

const NAME: &str = "iboardbot-web";
const VERSION: &str = env!("CARGO_PKG_VERSION");
const USAGE: &str = "
iBoardBot Web: Cloudless drawing fun.

Usage:
    iboardbot-web [-h] [-v] [-c <configfile>] [--headless] [--debug]

Example:

    iboardbot-web -c config.json

Options:
    -h --help        Show this screen.
    -v --version     Show version.
    -c <configfile>  Path to config file [default: config.json].
    --headless       Headless mode (start drawing immediately)
    --debug          Log debug logs
";

#[derive(Debug, Deserialize)]
struct Args {
    flag_c: String,
    flag_headless: bool,
    flag_debug: bool,
    flag_version: bool,
}

#[derive(RustEmbed)]
#[folder = "dist"]
struct Asset;

fn handle_embedded_file(path: &str) -> HttpResponse {
    match Asset::get(path) {
        Some(content) => HttpResponse::Ok()
            .content_type(mime_guess::from_path(path).first_or_octet_stream().as_ref())
            .body(content.data.into_owned()),
        None => HttpResponse::NotFound().body("404 Not Found"),
    }
}

#[actix_web::get("/static/{_:.*}")]
async fn static_files_handler(path: web::Path<String>) -> impl Responder {
    handle_embedded_file(path.as_str())
}

#[get("/config/")]
async fn config_handler(data: web::Data<State>) -> String {
    serde_json::to_value(&data.config)
        .expect("Could not serialize Config object")
        .to_string()
}

/// Return a list of SVG files from the SVG dir.
fn get_svg_files(dir: &str) -> Result<Vec<String>, io::Error> {
    let mut svg_files = read_dir(dir)
        // The `read_dir` function returns an iterator over results.
        // If any iterator entry fails, fail the whole iterator.
        .and_then(|iter| iter.collect::<Result<Vec<DirEntry>, io::Error>>())
        // Filter directory entries
        .map(|entries| {
            entries
                .iter()
                // Get filepath for entry
                .map(|entry| entry.path())
                // We only want files
                .filter(|path| path.is_file())
                // Map to filename
                .filter_map(|ref path| {
                    path.file_name()
                        .map(OsStr::to_os_string)
                        .and_then(|oss| oss.into_string().ok())
                })
                // We only want .svg files
                .filter(|filename| filename.ends_with(".svg"))
                // Collect vector of strings
                .collect::<Vec<String>>()
        })?;
    svg_files.sort();
    Ok(svg_files)
}

#[get("/list/")]
async fn list_handler(data: web::Data<State>) -> Result<web::Json<Vec<String>>, JsonError> {
    let svg_files = get_svg_files(&data.config.svg_dir).map_err(|_e| {
        JsonError::ServerError(ErrorDetails::from("Could not read files in SVG directory"))
    })?;
    Ok(web::Json(svg_files))
}

#[derive(Deserialize, Debug)]
struct PreviewRequest {
    svg: String,
}

#[derive(Deserialize, Debug)]
struct PrintRequest {
    svg: String,
    offset_x: f64,
    offset_y: f64,
    scale_x: f64,
    scale_y: f64,
    mode: PrintMode,
}

#[derive(Serialize, Debug)]
struct ErrorDetails {
    details: String,
}

impl ErrorDetails {
    fn from<S: Into<String>>(details: S) -> Self {
        ErrorDetails {
            details: details.into(),
        }
    }
}

#[derive(Debug)]
enum JsonError {
    ServerError(ErrorDetails),
    ClientError(ErrorDetails),
}

impl fmt::Display for JsonError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let val = serde_json::to_value(match self {
            JsonError::ServerError(details) => details,
            JsonError::ClientError(details) => details,
        });
        write!(f, "{}", val.expect("Could not serialize error details"))
    }
}
impl std::error::Error for JsonError {}
impl ResponseError for JsonError {
    fn error_response(&self) -> HttpResponse {
        let mut builder = match self {
            JsonError::ServerError(_) => HttpResponse::InternalServerError(),
            JsonError::ClientError(_) => HttpResponse::BadRequest(),
        };
        builder
            .content_type("application/json")
            .body(self.to_string())
    }
}

type JsonResult<T> = Result<T, JsonError>;

#[post("/preview/")]
async fn preview_handler(req: web::Json<PreviewRequest>) -> JsonResult<web::Json<Vec<Polyline>>> {
    match svg2polylines::parse(&req.svg, SVG2POLYLINES_TOLERANCE) {
        Ok(polylines) => Ok(web::Json(polylines)),
        Err(errmsg) => Err(JsonError::ClientError(ErrorDetails::from(errmsg))),
    }
}

#[post("/print/")]
async fn print_handler(
    data: web::Data<State>,
    print_request: web::Json<PrintRequest>,
) -> Result<HttpResponse, JsonError> {
    // Parse SVG into list of polylines
    info!("Requested print mode: {:?}", print_request.mode);
    let mut polylines = match svg2polylines::parse(&print_request.svg, SVG2POLYLINES_TOLERANCE) {
        Ok(polylines) => polylines,
        Err(e) => return Err(JsonError::ClientError(ErrorDetails::from(e))),
    };

    // Scale polylines
    scaling::scale_polylines(
        &mut polylines,
        (print_request.offset_x, print_request.offset_y),
        (print_request.scale_x, print_request.scale_y),
    );

    // Get access to queue
    let tx = data.robot_queue.lock().map_err(|e| {
        JsonError::ClientError(ErrorDetails::from(format!(
            "Could not communicate with robot thread: {}",
            e
        )))
    })?;
    let task = print_request.mode.to_print_task(polylines);
    tx.send(task).map_err(|e| {
        JsonError::ServerError(ErrorDetails::from(format!(
            "Could not send print request to robot thread: {}",
            e
        )))
    })?;

    info!("Printing...");
    Ok(HttpResponse::new(StatusCode::NO_CONTENT))
}

fn headless_start(robot_queue: RobotQueue, config: &Config) -> Result<(), HeadlessError> {
    // Get SVG files to be printed
    let svg_files = get_svg_files(&config.svg_dir)?;
    if svg_files.is_empty() {
        return Err(HeadlessError::NoFiles);
    }

    // Read SVG files
    let mut svgs = vec![];
    let base_path = Path::new(&config.svg_dir);
    for file in svg_files {
        let mut svg = String::new();
        let mut f = File::open(base_path.join(&file))?;
        f.read_to_string(&mut svg)?;
        svgs.push(svg);
    }

    // Specify target area bounds
    let mut bounds = Bounds {
        x: Range {
            min: 0.0,
            max: f64::from(robot::IBB_WIDTH),
        },
        y: Range {
            min: 0.0,
            max: f64::from(robot::IBB_HEIGHT),
        },
    };
    bounds.add_padding(5.0);

    // Parse SVG strings into lists of polylines
    let polylines_set: Vec<Vec<Polyline>> = svgs
        .iter()
        .map(|ref svg| {
            svg2polylines::parse(svg, SVG2POLYLINES_TOLERANCE)
                .map_err(|e| HeadlessError::SvgParse(e))
                .and_then(|mut polylines| {
                    scaling::fit_polylines(&mut polylines, &bounds)
                        .map_err(|e| HeadlessError::PolylineScale(e))?;
                    Ok(polylines)
                })
        })
        .collect::<Result<Vec<_>, HeadlessError>>()?;

    // Get access to queue
    let tx = robot_queue.lock().map_err(|e| {
        HeadlessError::Queue(format!("Could not communicate with robot thread: {}", e))
    })?;

    // Create print task
    let interval_duration = Duration::from_secs(config.interval_seconds);
    let task = PrintTask::Scheduled(interval_duration, polylines_set);

    // Send task to robot
    tx.send(task).map_err(|e| {
        HeadlessError::Queue(format!(
            "Could not send print request to robot thread: {}",
            e
        ))
    })?;

    info!("Printing...");
    Ok(())
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // Parse args
    let args: Args = Docopt::new(USAGE)
        .and_then(|d| d.deserialize())
        .unwrap_or_else(|e| e.exit());

    // Show version and exit
    if args.flag_version {
        println!("{} v{}", NAME, VERSION);
        process::exit(0);
    }

    // Init logger
    let log_level = if args.flag_debug {
        LevelFilter::Debug
    } else {
        LevelFilter::Info
    };
    if let Err(_) = TermLogger::init(
        log_level,
        LogConfig::default(),
        TerminalMode::Mixed,
        ColorChoice::Auto,
    ) {
        eprintln!("Could not initialize TermLogger. Falling back to SimpleLogger.");
        SimpleLogger::init(log_level, LogConfig::default())
            .expect("Could not initialize SimpleLogger");
    }

    // Headless mode
    let headless_mode: bool = args.flag_headless;

    // Parse config
    let configfile = File::open(&args.flag_c).unwrap_or_else(|e| {
        error!("Could not open configfile ({}): {}", &args.flag_c, e);
        abort(1);
    });
    let config: RawConfig = serde_json::from_reader(configfile).unwrap_or_else(|e| {
        error!("Could not parse configfile ({}): {}", &args.flag_c, e);
        abort(1);
    });

    // Check if this is an active config
    match Config::from(&config) {
        Some(c) => main_active(c, headless_mode).await,
        None => main_preview(PreviewConfig::from(&config)).await,
    }
}

/// Start the web server in active (printing) mode.
async fn main_active(config: Config, headless_mode: bool) -> std::io::Result<()> {
    info!("Starting server in active mode (with robot attached)");

    // Check for presence of relevant paths
    let device_path = Path::new(&config.device);
    if !device_path.exists() {
        error!("Device {} does not exist", &config.device);
        abort(2);
    }
    let svg_dir_path = Path::new(&config.svg_dir);
    if !svg_dir_path.exists() || !svg_dir_path.is_dir() {
        error!("SVG dir {} does not exist", &config.svg_dir);
        abort(2);
    }

    // Launch robot thread
    let baud_rate = BaudRate::Baud115200;
    let tx = robot::communicate(&config.device, baud_rate, config.time_limits);

    // Initialize server state
    let robot_queue = Arc::new(Mutex::new(tx));
    let state = web::Data::new(State {
        config: config.clone(),
        robot_queue: robot_queue.clone(),
    });

    // Print mode
    match headless_mode {
        true => info!("Starting in headless mode"),
        false => info!("Starting in normal mode"),
    };

    // If we're in headless mode, start the print jobs
    if headless_mode {
        headless_start(robot_queue.clone(), &config).unwrap_or_else(|e| {
            error!("Could not start headless mode: {}", e);
            abort(3);
        });
    }

    // Start web server
    let interface = config.listen.clone();
    info!("Listening on {}", interface);
    HttpServer::new(move || {
        let mut app = App::new()
            .app_data(state.clone())
            .service(static_files_handler)
            .service(config_handler)
            .service(list_handler)
            .service(preview_handler)
            .service(print_handler);
        if headless_mode {
            app = app.route(
                "/",
                web::get().to(|| async { handle_embedded_file("headless.html") }),
            );
        } else {
            // For development
            app = app.route(
                "/headless/",
                web::get().to(|| async { handle_embedded_file("headless.html") }),
            );
            app = app.route(
                "/",
                web::get().to(|| async { handle_embedded_file("index.html") }),
            );
        };
        app
    })
    .bind(interface)?
    .run()
    .await
}

/// Start the web server in preview-only mode.
async fn main_preview(config: PreviewConfig) -> std::io::Result<()> {
    info!("Starting server in preview-only mode");

    // Start web server
    let interface = config.listen.clone();
    info!("Listening on {}", interface);
    HttpServer::new(move || {
        App::new()
            .service(static_files_handler)
            .service(preview_handler)
            .route(
                "/",
                web::get().to(|| async { handle_embedded_file("index-preview.html") }),
            )
    })
    .bind(interface)?
    .run()
    .await
}

fn abort(exit_code: i32) -> ! {
    io::stdout().flush().expect("Could not flush stdout");
    io::stderr().flush().expect("Could not flush stderr");

    // No idea why this is required, but otherwise the error log doesn't show up :(
    sleep(Duration::from_millis(100));

    process::exit(exit_code);
}
