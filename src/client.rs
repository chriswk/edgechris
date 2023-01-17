use actix_web::web::{self, Json};
use unleash_types::client_features::ClientFeatures;

use crate::{AppState, ClientKey, EdgeResult};

pub async fn get_features(
    _client_key: ClientKey,
    app_state: web::Data<AppState>,
) -> EdgeResult<ClientFeatures> {
    Ok(Json(app_state.client_features.clone()))
}
