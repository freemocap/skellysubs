//! Reading requests execute through the workspace graph host.
use super::*;
use crate::{ai::results::Retained, language::reading::Request};

impl Application {
    pub(super) async fn shared_reading(
        self: &Arc<Self>,
        request: Arc<Request>,
    ) -> Result<Retained> {
        self.native_reading(request).await
    }
}
