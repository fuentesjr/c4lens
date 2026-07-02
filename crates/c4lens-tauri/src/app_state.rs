use std::sync::Mutex;

use crate::commands::repo::GenerationDiff;
use c4lens_core::RepoHandle;

use crate::model_watcher::ModelWatcherHandle;

#[derive(Default)]
pub struct AppState {
    pub active_repo: Mutex<Option<RepoHandle>>,
    pub model_watcher: Mutex<Option<ModelWatcherHandle>>,
    pub latest_generation_candidate: Mutex<Option<GenerationDiff>>,
}
