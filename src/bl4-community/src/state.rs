use std::collections::HashSet;

use crate::db::Database;
use crate::rate_limit::RateLimiter;

pub struct AppState {
    pub db: Database,
    pub write_limiter: RateLimiter,
    pub allowed_ips: HashSet<String>,
}
