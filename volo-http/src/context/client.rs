//! Context and its utilities of client

use std::time::Duration;

use chrono::{DateTime, Local};
use volo::{
    context::{Reusable, Role, RpcCx, RpcInfo},
    newtype_impl_context,
};

use crate::{
    client::Target,
    utils::macros::{impl_deref_and_deref_mut, stat_impl},
};

/// RPC context of http client
#[derive(Debug)]
pub struct ClientContext(pub(crate) RpcCx<ClientCxInner, Config>);

impl ClientContext {
    /// Create a new [`ClientContext`]
    pub fn new() -> Self {
        Self(RpcCx::new(
            RpcInfo::<Config>::with_role(Role::Client),
            ClientCxInner {
                target: Target::None,
                stats: ClientStats::default(),
            },
        ))
    }
}

impl Default for ClientContext {
    fn default() -> Self {
        Self::new()
    }
}

newtype_impl_context!(ClientContext, Config, 0);

impl_deref_and_deref_mut!(ClientContext, RpcCx<ClientCxInner, Config>, 0);

/// Inner details of [`ClientContext`]
#[derive(Debug)]
pub struct ClientCxInner {
    /// Target of the current request
    target: Target,

    /// Statistics of client
    ///
    /// This is unstable now and may be changed in the future.
    pub stats: ClientStats,
}

impl ClientCxInner {
    /// Get [`Target`] of current request context.
    pub fn target(&self) -> &Target {
        &self.target
    }

    pub(crate) fn set_target(&mut self, target: Target) {
        self.target = target;
    }
}

/// Statistics of client
///
/// This is unstable now and may be changed in the future.
#[derive(Debug, Default, Clone)]
pub struct ClientStats {
    transport_start_at: Option<DateTime<Local>>,
    transport_end_at: Option<DateTime<Local>>,
}

impl ClientStats {
    stat_impl!(transport_start_at);
    stat_impl!(transport_end_at);
}

/// Configuration of the request
#[derive(Clone, Debug, Default)]
pub struct Config {
    /// Timeout of the current request
    pub timeout: Option<Duration>,
}

impl Config {
    /// Create a default [`Config`]
    #[inline]
    pub fn new() -> Self {
        Default::default()
    }

    /// Get current timeout of the request
    #[inline]
    pub fn timeout(&self) -> Option<&Duration> {
        self.timeout.as_ref()
    }

    /// Set timeout to the request
    #[inline]
    pub fn set_timeout(&mut self, timeout: Option<Duration>) {
        self.timeout = timeout;
    }
}

impl Reusable for Config {
    fn clear(&mut self) {
        self.timeout = None;
    }
}
