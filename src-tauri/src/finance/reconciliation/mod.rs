//! Scoped enterprise financial-operations boundary.
//!
//! This module is reserved for deterministic, local-first helpers. It must not
//! approve financial decisions, transmit funds, or bypass Ghost's policy and
//! approval pipeline.

pub mod matcher;
