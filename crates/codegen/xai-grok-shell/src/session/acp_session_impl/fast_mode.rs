//! Session-actor Fast Mode mutation.
//!
//! This path changes one sampling field and deliberately has no model-selection
//! side effects.

use super::*;

impl SessionActor {
    pub(super) async fn handle_set_sampling_fast_mode(
        &self,
        enabled: bool,
    ) -> Result<String, acp::Error> {
        let Some(mut config) = self.chat_state_handle.get_sampling_config().await else {
            return Err(acp::Error::internal_error().data("session sampling config is unavailable"));
        };
        let live_model = config.model.clone();
        let models = self.models_manager.models();
        match crate::agent::config::find_model_by_id(&models, &live_model) {
            Some(model) if model.info().supports_fast_mode => {}
            Some(_) => {
                return Err(
                    acp::Error::invalid_params().data("current model does not support fast mode")
                );
            }
            None => {
                return Err(acp::Error::invalid_params()
                    .data("current session model is not in the catalog"));
            }
        }

        config.fast_mode = Some(enabled);
        self.chat_state_handle.update_sampling_config(config);
        Ok(live_model)
    }
}
