use std::future::{ready, Ready};

use actix_web::{dev::Payload, http::header::HeaderValue, web::Data, FromRequest, HttpRequest};
use tracing::info;

use crate::{AppState, ClientKey, EdgeError};

impl FromRequest for ClientKey {
    type Error = EdgeError;
    type Future = Ready<Result<Self, Self::Error>>;

    fn from_request(req: &HttpRequest, _payload: &mut Payload) -> Self::Future {
        if let Some(app_state) = req.app_data::<Data<AppState>>() {
            let value = req.headers().get("Authorization");
            let key = match value {
                Some(v) => ClientKey::try_from(v.clone()),
                None => Err(EdgeError::NoAuthorizationHeader),
            }
            .and_then(|client_key| {
                if app_state.valid_keys.contains(&client_key.key) {
                    Ok(client_key)
                } else {
                    Err(EdgeError::InvalidKey)
                }
            });
            ready(key)
        } else {
            info!("Could not extract app_data (why not)");
            ready(Err(EdgeError::AppStateError))
        }
    }
}

impl TryFrom<HeaderValue> for ClientKey {
    type Error = EdgeError;

    fn try_from(header_val: HeaderValue) -> Result<Self, Self::Error> {
        header_val
            .to_str()
            .map(|s| s.to_string())
            .map(|key| ClientKey { key })
            .map_err(|_| EdgeError::InvalidKey)
    }
}
