mod generation;
mod index;
mod loader;
mod lock;
mod model;
mod sample;

pub use generation::{
    build_minimal_generated_model, render_generated_model_yaml, BUNDLED_MODEL_SCHEMA_JSON,
    GENERATED_MODEL_HEADER, GENERATED_MODEL_PATH, SCHEMA_PATH,
};
pub use index::{default_index_path, get_element_code, migrate_index, scan_repo, ScanOptions};
pub use loader::{
    load_effective_model_from_repo, load_effective_model_from_repo_recovering_generated_overlay,
    validate_generated_overlay_yaml,
};
pub use lock::{acquire_repo_write_lock, RepoWriteLock};
pub use model::*;
pub use sample::hardcoded_sample_model;
