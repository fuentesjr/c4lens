use c4lens_core::{ScanSummary, ValidationReport};
use serde::Serialize;

pub const INDEX_UPDATED: &str = "index-updated";
pub const MODEL_CHANGED: &str = "model-changed";
pub const VALIDATION_FAILED: &str = "validation-failed";

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IndexUpdatedPayload {
    pub repo_id: String,
    pub summary: ScanSummary,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelChangedPayload {
    pub repo_id: String,
    pub source_sha: String,
    pub validation: ValidationReport,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ValidationFailedPayload {
    pub repo_id: String,
    pub validation: ValidationReport,
}
