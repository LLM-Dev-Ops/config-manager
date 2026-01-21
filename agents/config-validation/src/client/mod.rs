//! HTTP clients for external services
//!
//! This module provides HTTP clients for communicating with external services.
//! No direct SQL connections - all persistence is done through service APIs.

pub mod ruvector;

pub use ruvector::RuvectorClient;
