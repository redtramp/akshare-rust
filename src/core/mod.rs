//! 核心基础设施层。

pub mod config;
pub mod df;
pub mod error;
pub mod http;
pub mod js_engine;

pub use config::{set_proxies, AkshareConfig, ProxyContext};
pub use df::Df;
pub use error::{AkshareError, Result};
