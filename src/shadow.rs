use crate::EdgeResult;
use actix_web::web::Json;
use serde::Serialize;
use shadow_rs::shadow;

shadow!(build);

#[derive(Serialize, Debug)]
pub struct PkgInfo {
    pub version: String,
    pub major: String,
    pub minor: String,
    pub patch: String,
    pub pre: String,
}

impl Default for PkgInfo {
    fn default() -> Self {
        Self {
            version: build::PKG_VERSION.into(),
            major: build::PKG_VERSION_MAJOR.into(),
            minor: build::PKG_VERSION_MINOR.into(),
            patch: build::PKG_VERSION_PATCH.into(),
            pre: build::PKG_VERSION_PRE.into(),
        }
    }
}

#[derive(Serialize, Debug)]
pub struct ShadowInfo {
    pub version: String,
    pub long_version: String,
    pub pkg: PkgInfo,
}

impl Default for ShadowInfo {
    fn default() -> Self {
        Self {
            version: build::VERSION.into(),
            long_version: build::CLAP_LONG_VERSION.into(),
            pkg: PkgInfo::default(),
        }
    }
}

pub async fn info() -> EdgeResult<ShadowInfo> {
    Ok(Json(ShadowInfo::default()))
}
