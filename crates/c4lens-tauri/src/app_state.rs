use std::sync::Mutex;

use c4lens_core::RepoHandle;

#[derive(Default)]
pub struct AppState {
    pub active_repo: Mutex<Option<RepoHandle>>,
}
