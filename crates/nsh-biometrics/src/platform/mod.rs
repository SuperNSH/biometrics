//! The platform backends. Each target gets one; the rest report that nothing is available.

mod unsupported;

use std::sync::Arc;

use crate::backend::Backend;

/// The backend of the target this build runs on.
pub(crate) fn default_backend() -> Arc<dyn Backend> {
    Arc::new(unsupported::Unsupported)
}
