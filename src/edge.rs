use std::collections::HashMap;

use crate::{AppState, ClientKey, EdgeResult};
use actix_web::web::{self, Json};
use serde::{Deserialize, Serialize};
use tracing::info;
use unleash_types::frontend::{EvaluatedToggle, EvaluatedVariant, FrontendResult};
use unleash_yggdrasil::{state::InnerContext, EngineState, VariantDef};

pub async fn get_active_features(
    _client_key: ClientKey,
    unleash_context: web::Query<UnleashContext>,
    app_state: web::Data<AppState>,
) -> EdgeResult<FrontendResult> {
    let context = unleash_context.into_inner();
    info!("{:#?}", context);
    let toggles = evaluate_features(context, app_state)
        .into_iter()
        .filter(|e| e.enabled)
        .collect();
    Ok(Json(FrontendResult { toggles }))
}

pub async fn active_features(
    _client_key: ClientKey,
    unleash_context: web::Json<UnleashContext>,
    app_state: web::Data<AppState>,
) -> EdgeResult<FrontendResult> {
    let context = unleash_context.into_inner();
    info!("{:#?}", context);
    let toggles = evaluate_features(context, app_state)
        .into_iter()
        .filter(|e| e.enabled)
        .collect();
    Ok(Json(FrontendResult { toggles }))
}
pub async fn get_all_features(
    _client_key: ClientKey,
    unleash_context: web::Query<UnleashContext>,
    app_state: web::Data<AppState>,
) -> EdgeResult<FrontendResult> {
    let toggles = evaluate_features(unleash_context.into_inner(), app_state);
    Ok(Json(FrontendResult { toggles }))
}

pub async fn all_features(
    _client_key: ClientKey,
    unleash_context: web::Json<UnleashContext>,
    app_state: web::Data<AppState>,
) -> EdgeResult<FrontendResult> {
    let toggles = evaluate_features(unleash_context.into_inner(), app_state);
    Ok(Json(FrontendResult { toggles }))
}
fn evaluate_features(
    context: UnleashContext,
    app_state: web::Data<AppState>,
) -> Vec<EvaluatedToggle> {
    let mut engine = EngineState::new();
    engine.take_state(app_state.client_features.clone());
    let inner_context: InnerContext = context.to_inner_context();
    app_state
        .client_features
        .to_owned()
        .features
        .into_iter()
        .map(|cf| EvaluatedToggle {
            name: cf.name.clone(),
            enabled: engine.is_enabled(cf.name.clone(), &inner_context),
            variant: engine
                .get_variant(cf.name, &inner_context)
                .to_evaluated_variant(),
            impression_data: cf.impression_data.unwrap_or(false),
        })
        .collect()
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct UnleashContext {
    pub user_id: Option<String>,
    pub session_id: Option<String>,
    pub environment: Option<String>,
    pub app_name: Option<String>,
    pub current_time: Option<String>,
    pub remote_address: Option<String>,
    pub properties: Option<HashMap<String, String>>,
}

pub trait ToInnerContext {
    fn to_inner_context(&self) -> InnerContext;
}
pub trait ToEvaluatedVariant {
    fn to_evaluated_variant(&self) -> EvaluatedVariant;
}
impl ToEvaluatedVariant for VariantDef {
    fn to_evaluated_variant(&self) -> EvaluatedVariant {
        EvaluatedVariant {
            name: self.name.clone(),
            enabled: self.enabled,
            payload: self.payload.clone(),
        }
    }
}

impl ToInnerContext for UnleashContext {
    fn to_inner_context(&self) -> InnerContext {
        InnerContext {
            user_id: self.user_id.clone(),
            session_id: self.session_id.clone(),
            environment: self.environment.clone(),
            app_name: self.app_name.clone(),
            current_time: self.current_time.clone(),
            remote_address: self.remote_address.clone(),
            properties: self.properties.clone(),
        }
    }
}
