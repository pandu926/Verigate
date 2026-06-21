//! Credential verification pipeline.
//!
//! Provides W3C VC models, trait-based verifiers for 4 credential types,
//! JWT signature verification, trusted issuer registry, SD-JWT parsing,
//! and DisclosedFact normalization.

pub mod issuer_trust;
pub mod models;
pub mod normalizer;
pub mod sd_jwt;
pub mod verifier;
