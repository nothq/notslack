use std::sync::Arc;

use crate::model::SlackReactionCatalogSnapshot;

use super::SlackLiveWorkspaceLoader;

impl SlackLiveWorkspaceLoader {
    pub fn load_reaction_catalog(&self) -> Result<Arc<SlackReactionCatalogSnapshot>, String> {
        if let Some(snapshot) = self.cached_reaction_catalog()? {
            return Ok(snapshot);
        }

        let _fetch_guard = self
            .reaction_catalog_fetch_lock
            .lock()
            .map_err(|_| "Slack reaction catalog fetch mutex poisoned".to_string())?;
        if let Some(snapshot) = self.cached_reaction_catalog()? {
            return Ok(snapshot);
        }

        let snapshot = Arc::new(self.api.load_reaction_catalog(&self.team_id)?);
        *self
            .reaction_catalog_cache
            .lock()
            .map_err(|_| "Slack reaction catalog cache mutex poisoned".to_string())? =
            Some(Arc::clone(&snapshot));
        Ok(snapshot)
    }

    pub(super) fn invalidate_reaction_catalog(&self) {
        let _fetch_guard = self
            .reaction_catalog_fetch_lock
            .lock()
            .expect("Slack reaction catalog fetch mutex poisoned");
        *self
            .reaction_catalog_cache
            .lock()
            .expect("Slack reaction catalog cache mutex poisoned") = None;
    }

    fn cached_reaction_catalog(&self) -> Result<Option<Arc<SlackReactionCatalogSnapshot>>, String> {
        self.reaction_catalog_cache
            .lock()
            .map_err(|_| "Slack reaction catalog cache mutex poisoned".to_string())
            .map(|snapshot| snapshot.clone())
    }
}
