//! TOML configuration — Phase 8. CLI overrides config (`plan.md`).

#![allow(dead_code)] // exercised in Phase 8

/// Placeholder until serde/toml merge is implemented.
#[derive(Debug, Clone, Default)]
pub struct Config;

impl Config {
    pub fn load_default() -> Self {
        Self
    }
}
