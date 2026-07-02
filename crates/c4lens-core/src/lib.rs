mod generation;
mod index;
mod loader;
mod lock;
mod model;
mod sample;

pub use generation::{
    build_minimal_generated_model, build_minimal_generated_model_from_authored_system,
    render_generated_model_yaml, single_authored_internal_system_for_generation,
    BUNDLED_MODEL_SCHEMA_JSON, GENERATED_MODEL_HEADER, GENERATED_MODEL_PATH, SCHEMA_PATH,
};
pub use index::{
    default_index_path, get_element_code, list_internal_crate_import_edges, migrate_index,
    scan_repo, ScanOptions,
};
pub use loader::{
    load_effective_model_from_repo, load_effective_model_from_repo_recovering_generated_overlay,
    validate_generated_overlay_yaml, validate_generated_overlay_yaml_with_report,
};
pub use lock::{acquire_repo_write_lock, RepoWriteLock};
pub use model::*;
pub use sample::hardcoded_sample_model;
