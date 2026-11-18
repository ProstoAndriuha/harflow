use harflow_core::HarflowError;
use serde::{Deserialize, Serialize};

/// Execution modes defined by SRS M13.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ReplayMode {
    #[default]
    Mock,
    AuthorizedLive,
}

/// Security policy controlling replay validation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReplayConfig {
    pub mode: ReplayMode,
    pub offline_only: bool,
    pub allowlist_hosts: Vec<String>,
    pub allow_mutating_methods: bool,
    pub dry_run: bool,
}

impl Default for ReplayConfig {
    fn default() -> Self {
        Self {
            mode: ReplayMode::Mock,
            offline_only: true,
            allowlist_hosts: Vec::new(),
            allow_mutating_methods: false,
            dry_run: true,
        }
    }
}

/// Summary report emitted after replay checks.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ValidationReport {
    pub total_checks: usize,
    pub passed: usize,
    pub failed: usize,
    pub skipped: usize,
}

/// Validates whether a replay request satisfies HARFlow's mock-first and allowlist rules.
pub fn validate_replay_policy(
    config: &ReplayConfig,
    method: &str,
    host: &str,
) -> Result<(), HarflowError> {
    if config.mode == ReplayMode::Mock {
        return Ok(());
    }

    if config.offline_only {
        return Err(HarflowError::SecurityViolation(
            "live replay is blocked while offline_only is enabled".to_string(),
        ));
    }

    if !config
        .allowlist_hosts
        .iter()
        .any(|allowed| allowed.eq_ignore_ascii_case(host))
    {
        return Err(HarflowError::SecurityViolation(format!(
            "host '{host}' is not in the explicit replay allowlist"
        )));
    }

    let is_mutating = !matches!(
        method.to_ascii_uppercase().as_str(),
        "GET" | "HEAD" | "OPTIONS"
    );
    if is_mutating && !config.allow_mutating_methods {
        return Err(HarflowError::SecurityViolation(format!(
            "mutating HTTP method '{method}' requires explicit permission"
        )));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blocks_unallowlisted_or_mutating_live_replay() {
        let default_cfg = ReplayConfig::default();
        assert!(validate_replay_policy(&default_cfg, "POST", "api.example.test").is_ok());

        let live_offline = ReplayConfig {
            mode: ReplayMode::AuthorizedLive,
            offline_only: true,
            allowlist_hosts: vec!["api.example.test".into()],
            allow_mutating_methods: false,
            dry_run: false,
        };
        assert_eq!(
            validate_replay_policy(&live_offline, "GET", "api.example.test")
                .unwrap_err()
                .exit_code()
                .as_i32(),
            7
        );

        let live_no_mut = ReplayConfig {
            mode: ReplayMode::AuthorizedLive,
            offline_only: false,
            allowlist_hosts: vec!["api.example.test".into()],
            allow_mutating_methods: false,
            dry_run: false,
        };
        assert!(validate_replay_policy(&live_no_mut, "GET", "api.example.test").is_ok());
        assert_eq!(
            validate_replay_policy(&live_no_mut, "POST", "api.example.test")
                .unwrap_err()
                .exit_code()
                .as_i32(),
            7
        );
    }
}
