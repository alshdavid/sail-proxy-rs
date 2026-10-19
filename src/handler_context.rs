use std::sync::Arc;

use crate::config::Config;

#[derive(Clone)]
pub struct HandlerContext {
  pub config: Arc<Config>,
}
