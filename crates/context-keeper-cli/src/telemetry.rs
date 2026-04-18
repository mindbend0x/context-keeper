//! Opt-in, anonymized telemetry for the `context-keeper` CLI.
//!
//! Design goals (FZ-87):
//! * **Opt-in only.** On first run, if no config exists, prompt the user.
//!   Piped / non-TTY stdin defaults to disabled without prompting.
//! * **Anonymous.** A random UUID install-id is generated once and persisted.
//!   No paths, no config values, no error messages are emitted.
//! * **Self-hosted.** Users point events anywhere via the standard
//!   `OTEL_EXPORTER_OTLP_ENDPOINT` / `OTEL_EXPORTER_OTLP_HEADERS` /
//!   `OTEL_SERVICE_NAME` env vars. We ship no default endpoint.
//! * **Kill switch.** `CK_TELEMETRY_DISABLE=1` disables emission regardless
//!   of stored consent.
//!
//! Emitted events: `cli.install`, `cli.invoke`, `cli.error`.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::io::{self, IsTerminal, Write};
use std::path::PathBuf;
use uuid::Uuid;

use opentelemetry::global;
use opentelemetry::trace::{Span, Tracer};
use opentelemetry::KeyValue;
use opentelemetry_otlp::SpanExporter;
use opentelemetry_sdk::trace::TracerProvider;
use opentelemetry_sdk::Resource;

/// Environment variable that hard-disables telemetry regardless of consent.
pub const DISABLE_ENV_VAR: &str = "CK_TELEMETRY_DISABLE";

/// Service name used when `OTEL_SERVICE_NAME` is not set.
pub const DEFAULT_SERVICE_NAME: &str = "context-keeper-cli";

/// Persisted config file format (`~/.context-keeper/config.toml`).
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub telemetry: TelemetryConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelemetryConfig {
    /// Whether the user opted in to anonymous telemetry.
    pub enabled: bool,
    /// Random anonymous identifier, generated once on first run.
    pub install_id: String,
}

impl Default for TelemetryConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            install_id: Uuid::new_v4().to_string(),
        }
    }
}

/// Resolve `~/.context-keeper/` (or `$XDG_CONFIG_HOME/context-keeper/`).
///
/// If `XDG_CONFIG_HOME` is set we honour it; otherwise we fall back to
/// `$HOME/.context-keeper/` for consistency with the rest of the project
/// (the `STORAGE_BACKEND` default also lives under `~/.context-keeper/`).
pub fn config_dir() -> Result<PathBuf> {
    if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
        if !xdg.is_empty() {
            return Ok(PathBuf::from(xdg).join("context-keeper"));
        }
    }
    let home = dirs::home_dir().context("could not determine $HOME")?;
    Ok(home.join(".context-keeper"))
}

pub fn config_path() -> Result<PathBuf> {
    Ok(config_dir()?.join("config.toml"))
}

/// Load config from disk if present.
pub fn load_config() -> Result<Option<Config>> {
    let path = config_path()?;
    if !path.exists() {
        return Ok(None);
    }
    let raw = std::fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
    let cfg: Config = toml::from_str(&raw).with_context(|| format!("parse {}", path.display()))?;
    Ok(Some(cfg))
}

/// Persist the given config to `~/.context-keeper/config.toml`, creating
/// the parent directory if it doesn't exist.
pub fn save_config(cfg: &Config) -> Result<()> {
    let dir = config_dir()?;
    std::fs::create_dir_all(&dir).with_context(|| format!("create {}", dir.display()))?;
    let path = dir.join("config.toml");
    let body = toml::to_string_pretty(cfg).context("serialize config")?;
    std::fs::write(&path, body).with_context(|| format!("write {}", path.display()))?;
    Ok(())
}

/// Outcome of the first-run consent flow.
pub struct ConsentOutcome {
    pub config: Config,
    /// True when the config did not previously exist; used to emit a
    /// one-time `cli.install` event after the first persisted consent.
    pub first_run: bool,
}

/// Resolve consent: load existing config, else prompt (TTY) or default to
/// disabled (non-TTY), and persist the result.
pub fn resolve_consent() -> Result<ConsentOutcome> {
    if let Some(existing) = load_config()? {
        return Ok(ConsentOutcome {
            config: existing,
            first_run: false,
        });
    }

    // Fresh install. Default to disabled; prompt only if stdin is a TTY.
    let enabled = if io::stdin().is_terminal() && io::stderr().is_terminal() {
        prompt_for_consent().unwrap_or(false)
    } else {
        false
    };

    let cfg = Config {
        telemetry: TelemetryConfig {
            enabled,
            install_id: Uuid::new_v4().to_string(),
        },
    };
    save_config(&cfg)?;
    Ok(ConsentOutcome {
        config: cfg,
        first_run: true,
    })
}

fn prompt_for_consent() -> Result<bool> {
    eprint!(
        "\nContext Keeper can send anonymous usage events (subcommand name, \n\
         version, OS/arch, error class) to the OTLP endpoint configured via \n\
         OTEL_EXPORTER_OTLP_ENDPOINT. No paths, arguments or error messages \n\
         are transmitted. See README for details.\n\
         \n\
         Enable anonymous telemetry? [y/N] "
    );
    io::stderr().flush().ok();

    let mut answer = String::new();
    io::stdin().read_line(&mut answer)?;
    Ok(answer.trim().eq_ignore_ascii_case("y"))
}

/// True when telemetry should actually emit events for this process.
///
/// Honors both the persisted consent and the `CK_TELEMETRY_DISABLE`
/// override (which always wins).
pub fn is_active(cfg: &Config) -> bool {
    if env_is_truthy(DISABLE_ENV_VAR) {
        return false;
    }
    cfg.telemetry.enabled
}

fn env_is_truthy(key: &str) -> bool {
    match std::env::var(key) {
        Ok(v) => matches!(v.trim(), "1" | "true" | "TRUE" | "yes" | "YES"),
        Err(_) => false,
    }
}

/// Handle for the active OpenTelemetry pipeline. Dropping it flushes and
/// shuts down exporters cleanly.
pub struct TelemetryHandle {
    provider: Option<TracerProvider>,
    install_id: String,
    active: bool,
}

impl TelemetryHandle {
    /// Inert handle that emits nothing (used when telemetry is disabled).
    pub fn disabled(install_id: String) -> Self {
        Self {
            provider: None,
            install_id,
            active: false,
        }
    }

    #[allow(dead_code)] // Public API; only used from `#[cfg(test)]` in this crate.
    pub fn is_active(&self) -> bool {
        self.active
    }

    #[allow(dead_code)] // Public API; reserved for callers.
    pub fn install_id(&self) -> &str {
        &self.install_id
    }

    /// Emit `cli.invoke` with subcommand name, CLI version, OS, and arch.
    pub fn record_invoke(&self, subcommand: &str) {
        if !self.active {
            return;
        }
        let tracer = global::tracer(DEFAULT_SERVICE_NAME);
        let mut span = tracer.start("cli.invoke");
        span.set_attribute(KeyValue::new("subcommand", subcommand.to_string()));
        span.set_attribute(KeyValue::new("version", env!("CARGO_PKG_VERSION")));
        span.set_attribute(KeyValue::new("os", std::env::consts::OS));
        span.set_attribute(KeyValue::new("arch", std::env::consts::ARCH));
        span.set_attribute(KeyValue::new("install_id", self.install_id.clone()));
        span.end();
    }

    /// Emit `cli.install` (first-run only).
    pub fn record_install(&self) {
        if !self.active {
            return;
        }
        let tracer = global::tracer(DEFAULT_SERVICE_NAME);
        let mut span = tracer.start("cli.install");
        span.set_attribute(KeyValue::new("version", env!("CARGO_PKG_VERSION")));
        span.set_attribute(KeyValue::new("os", std::env::consts::OS));
        span.set_attribute(KeyValue::new("arch", std::env::consts::ARCH));
        span.set_attribute(KeyValue::new("install_id", self.install_id.clone()));
        span.end();
    }

    /// Emit `cli.error` with a classification string (e.g. the error
    /// enum variant or `"unknown"`). **Never** pass `.to_string()` of the
    /// error here — that may leak paths or user data.
    pub fn record_error(&self, error_class: &str) {
        if !self.active {
            return;
        }
        let tracer = global::tracer(DEFAULT_SERVICE_NAME);
        let mut span = tracer.start("cli.error");
        span.set_attribute(KeyValue::new("error_class", error_class.to_string()));
        span.set_attribute(KeyValue::new("version", env!("CARGO_PKG_VERSION")));
        span.set_attribute(KeyValue::new("os", std::env::consts::OS));
        span.set_attribute(KeyValue::new("arch", std::env::consts::ARCH));
        span.set_attribute(KeyValue::new("install_id", self.install_id.clone()));
        span.end();
    }

    /// Shut down exporters, flushing any pending spans. Best-effort; errors
    /// are swallowed because telemetry failure must never surface to the user.
    pub fn shutdown(mut self) {
        if let Some(provider) = self.provider.take() {
            let _ = provider.shutdown();
        }
    }
}

impl Drop for TelemetryHandle {
    fn drop(&mut self) {
        if let Some(provider) = self.provider.take() {
            let _ = provider.shutdown();
        }
    }
}

/// Initialise telemetry. Returns an inert handle when telemetry is disabled
/// (kill switch set, consent not granted) or when exporter setup fails —
/// never propagates telemetry errors to the caller.
pub fn init(cfg: &Config) -> TelemetryHandle {
    let install_id = cfg.telemetry.install_id.clone();
    if !is_active(cfg) {
        return TelemetryHandle::disabled(install_id);
    }

    match build_provider(&install_id) {
        Ok(provider) => {
            global::set_tracer_provider(provider.clone());
            TelemetryHandle {
                provider: Some(provider),
                install_id,
                active: true,
            }
        }
        Err(err) => {
            tracing::debug!("telemetry init failed: {err}");
            TelemetryHandle::disabled(install_id)
        }
    }
}

fn build_provider(install_id: &str) -> Result<TracerProvider> {
    let service_name =
        std::env::var("OTEL_SERVICE_NAME").unwrap_or_else(|_| DEFAULT_SERVICE_NAME.to_string());

    // The tonic builder reads OTEL_EXPORTER_OTLP_ENDPOINT and
    // OTEL_EXPORTER_OTLP_HEADERS from the environment by default.
    let exporter = SpanExporter::builder()
        .with_tonic()
        .build()
        .context("build OTLP span exporter")?;

    let resource = Resource::new(vec![
        KeyValue::new("service.name", service_name),
        KeyValue::new("service.version", env!("CARGO_PKG_VERSION")),
        KeyValue::new("telemetry.install_id", install_id.to_string()),
    ]);

    let provider = TracerProvider::builder()
        .with_batch_exporter(exporter, opentelemetry_sdk::runtime::Tokio)
        .with_resource(resource)
        .build();

    Ok(provider)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disabled_by_default_config() {
        let cfg = Config::default();
        assert!(!cfg.telemetry.enabled);
        // install_id populated
        assert!(!cfg.telemetry.install_id.is_empty());
    }

    #[test]
    fn kill_switch_overrides_consent() {
        let cfg = Config {
            telemetry: TelemetryConfig {
                enabled: true,
                install_id: "fixed".into(),
            },
        };
        // Simulate the env var being set.
        std::env::set_var(DISABLE_ENV_VAR, "1");
        assert!(!is_active(&cfg));
        std::env::remove_var(DISABLE_ENV_VAR);
        assert!(is_active(&cfg));
    }

    #[test]
    fn disabled_handle_is_inert() {
        let handle = TelemetryHandle::disabled("x".into());
        assert!(!handle.is_active());
        // None of these should panic or produce output.
        handle.record_install();
        handle.record_invoke("search");
        handle.record_error("SomeError");
    }

    #[test]
    fn config_roundtrip() {
        let cfg = Config {
            telemetry: TelemetryConfig {
                enabled: true,
                install_id: "abc-123".into(),
            },
        };
        let encoded = toml::to_string_pretty(&cfg).unwrap();
        assert!(encoded.contains("enabled = true"));
        assert!(encoded.contains("install_id"));
        let decoded: Config = toml::from_str(&encoded).unwrap();
        assert!(decoded.telemetry.enabled);
        assert_eq!(decoded.telemetry.install_id, "abc-123");
    }

    #[test]
    fn env_truthy_recognises_common_values() {
        for (val, expected) in [
            ("1", true),
            ("true", true),
            ("TRUE", true),
            ("yes", true),
            ("0", false),
            ("false", false),
            ("", false),
        ] {
            std::env::set_var("CK_TELEMETRY_TEST_VAR", val);
            assert_eq!(
                env_is_truthy("CK_TELEMETRY_TEST_VAR"),
                expected,
                "val={val}"
            );
        }
        std::env::remove_var("CK_TELEMETRY_TEST_VAR");
    }
}
