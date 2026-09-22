//! Shared controller ownership; domain authority and checks remain in adapters.
//! See THIRD_PARTY_NOTICES.md for Conary's demonstrated controller invariants.
pub mod comparison;
pub mod context;
pub mod contract;
pub mod controller;
pub mod decision;
pub mod evidence;
pub mod interaction;
pub mod jev;
pub mod process;
pub use contract::*;
pub use controller::run;
pub use tokio_util::sync::CancellationToken;
