use std::{
    fmt::{Display, Formatter},
    fs::File,
    io::BufReader,
    path::PathBuf,
};

use actix_web::{
    body::BoxBody,
    http::StatusCode,
    middleware,
    web::{self, Json},
    App, HttpResponse, HttpServer, ResponseError,
};
use actix_web_opentelemetry::RequestTracing;

use actix_tls::accept::rustls::reexports::ServerConfig;
use clap::{Args, Parser};
use rustls::{Certificate, PrivateKey};
use rustls_pemfile::{certs, pkcs8_private_keys};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tracing::info;
use unleash_types::client_features::ClientFeatures;
mod client;
mod edge;
mod extractors;
mod metrics;
mod shadow;

#[derive(Deserialize, Serialize, Debug, Clone, PartialEq, Eq)]
pub enum EdgeError {
    AppStateError,
    NoAuthorizationHeader,
    InvalidKey,
    NoFeaturesFile,
    NoServerCert,
    FeaturesFileReadError,
    NoServerKey,
    TlsError,
    UnleashContextQueryExtractionError,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ClientKey {
    pub key: String,
}

#[derive(Clone, Debug)]
pub struct AppState {
    pub valid_keys: Vec<String>,
    pub client_features: ClientFeatures,
}

pub fn read_features(file_to_read: PathBuf) -> Result<ClientFeatures, EdgeError> {
    let file = File::open(file_to_read).map_err(|_| EdgeError::NoFeaturesFile)?;
    let reader = BufReader::new(file);
    let f = serde_json::from_reader(reader).map_err(|_| EdgeError::FeaturesFileReadError)?;
    Ok(f)
}

impl AppState {
    pub fn build(config: CliArgs) -> Result<Self, EdgeError> {
        let client_features = read_features(config.features_file)?;
        Ok(AppState {
            valid_keys: config.client_keys,
            client_features,
        })
    }
}

impl Display for EdgeError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:#?}", self)
    }
}

impl ResponseError for EdgeError {
    fn status_code(&self) -> actix_web::http::StatusCode {
        match self {
            EdgeError::NoAuthorizationHeader => StatusCode::UNAUTHORIZED,
            EdgeError::InvalidKey => StatusCode::FORBIDDEN,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    fn error_response(&self) -> HttpResponse<BoxBody> {
        HttpResponse::build(self.status_code()).body(self.to_string())
    }
}

pub type EdgeResult<T> = Result<Json<T>, EdgeError>;

#[derive(Parser, Debug, Serialize, Deserialize, Clone)]
pub struct CliArgs {
    #[clap(flatten)]
    pub http_server: HttpServerArgs,

    /// A json file containing Client features in the Unleash syntax
    #[clap(short, long, env)]
    pub features_file: PathBuf,

    /// Accepted client keys
    #[clap(short, long, env)]
    pub client_keys: Vec<String>,
}

#[derive(Args, Debug, Clone, Serialize, Deserialize)]
pub struct TlsOptions {
    #[clap(env, long, default_value_t = false)]
    pub tls_enable: bool,
    #[clap(env, short = 'k', long)]
    pub tls_server_key: Option<PathBuf>,
    #[clap(env, long)]
    pub tls_server_cert: Option<PathBuf>,

    #[clap(env, long, default_value_t = 5443)]
    pub tls_server_port: u16,
}

#[derive(Args, Debug, Serialize, Deserialize, Clone)]
pub struct HttpServerArgs {
    #[clap(short, long, env, default_value_t = 5080)]
    pub port: u16,
    #[clap(short, long, env, default_value = "0.0.0.0")]
    pub interface: String,
    #[clap(flatten)]
    pub tls_opts: TlsOptions,
}

fn configure_tls(args: HttpServerArgs) -> Result<ServerConfig, EdgeError> {
    if args.tls_opts.tls_enable {
        let config = ServerConfig::builder()
            .with_safe_defaults()
            .with_no_client_auth();
        let mut cert_file = BufReader::new(
            File::open(
                args.tls_opts
                    .tls_server_cert
                    .expect("No TLS server cert")
                    .as_path(),
            )
            .map_err(|_| EdgeError::NoServerCert)?,
        );
        let mut key_file = BufReader::new(
            File::open(
                args.tls_opts
                    .tls_server_key
                    .expect("No server key")
                    .as_path(),
            )
            .expect("Could not read cert file"),
        );
        let cert_chain = certs(&mut cert_file)
            .expect("Could not build cert chain")
            .into_iter()
            .map(Certificate)
            .collect();
        let mut keys: Vec<PrivateKey> = pkcs8_private_keys(&mut key_file)
            .expect("Could not build pkcs8 private keys")
            .into_iter()
            .map(PrivateKey)
            .collect();
        config
            .with_single_cert(cert_chain, keys.remove(0))
            .map_err(|_e| EdgeError::TlsError)
    } else {
        Err(EdgeError::TlsError)
    }
}

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    dotenv::dotenv().ok();
    let args = CliArgs::parse();
    let app_state = AppState::build(args.clone()).expect("Couldn't build app state");
    let (metrics_handler, request_metrics) = metrics::instantiate(None);
    let server = HttpServer::new(move || {
        App::new()
            .wrap(actix_middleware_etag::Etag::default())
            .wrap(RequestTracing::new())
            .wrap(request_metrics.clone())
            .wrap(middleware::Logger::default())
            .app_data(web::Data::new(app_state.clone()))
            .service(
                web::scope("/internal-backstage")
                    .service(
                        web::resource("/metrics").route(web::get().to(metrics_handler.clone())),
                    )
                    .service(web::resource("/health").route(web::get().to(|| async {
                        HttpResponse::Ok().json(json!({
                            "status": "OK"
                        }))
                    })))
                    .service(web::resource("/info").route(web::get().to(shadow::info))),
            )
            .service(
                web::scope("/api/client")
                    .service(web::resource("/features").route(web::get().to(client::get_features))),
            )
            .service(
                web::scope("/api/proxy")
                    .service(
                        web::resource("")
                            .route(web::get().to(edge::get_active_features))
                            .route(web::post().to(edge::active_features)),
                    )
                    .service(
                        web::resource("/all")
                            .route(web::get().to(edge::get_all_features))
                            .route(web::post().to(edge::all_features)),
                    ),
            )
            .service(
                web::scope("/api/frontend")
                    .service(
                        web::resource("")
                            .route(web::get().to(edge::get_active_features))
                            .route(web::post().to(edge::active_features)),
                    )
                    .service(
                        web::resource("/all")
                            .route(web::get().to(edge::get_all_features))
                            .route(web::post().to(edge::all_features)),
                    ),
            )
    });

    let server = {
        if let Ok(https_config) = configure_tls(args.http_server.clone()) {
            server
                .bind_rustls(
                    (
                        args.http_server.interface.clone(),
                        args.http_server.tls_opts.tls_server_port,
                    ),
                    https_config,
                )?
                .bind((args.http_server.interface.clone(), args.http_server.port))
        } else {
            server.bind((args.http_server.interface.clone(), args.http_server.port))
        }
    }
    .unwrap_or_else(|_| {
        panic!(
            "Could not bind to {}:{}",
            args.http_server.interface, args.http_server.port
        );
    })
    .shutdown_timeout(5); // Graceful shutdown waits for existing connections for up to n seconds

    tokio::select! {
        _ = server.run() => {
            info!("Server received shutdown signal and is shutting down. Bye bye!");
        }
    }

    Ok(())
}
