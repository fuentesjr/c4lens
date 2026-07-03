use std::sync::Mutex;

use c4lens_core::CommandError;

use crate::commands::repo::GenerationDiff;

#[derive(Default)]
pub struct GenerationCandidateStore {
    latest: Mutex<Option<GenerationDiff>>,
}

impl GenerationCandidateStore {
    pub fn store(&self, candidate: GenerationDiff) -> Result<(), CommandError> {
        let mut guard = self.lock_for_update()?;
        *guard = Some(candidate);
        Ok(())
    }

    pub fn current(&self) -> Result<GenerationDiff, CommandError> {
        let guard = self.latest.lock().map_err(|_| {
            CommandError::new("fs.read_failed", "Failed to inspect generation candidates.")
        })?;
        guard.clone().ok_or_else(|| {
            CommandError::new(
                "generation.candidate_not_found",
                "No generation candidate is available.",
            )
        })
    }

    pub fn clear(&self) -> Result<(), CommandError> {
        let mut guard = self.lock_for_update()?;
        *guard = None;
        Ok(())
    }

    fn lock_for_update(
        &self,
    ) -> Result<std::sync::MutexGuard<'_, Option<GenerationDiff>>, CommandError> {
        self.latest
            .lock()
            .map_err(|_| CommandError::new("fs.write_failed", "Failed to update app state."))
    }
}

#[cfg(test)]
mod tests {
    use super::GenerationCandidateStore;

    #[test]
    fn current_reports_missing_when_empty() {
        let store = GenerationCandidateStore::default();
        let error = store.current().expect_err("candidate should be missing");

        assert_eq!(error.code, "generation.candidate_not_found");
    }
}
