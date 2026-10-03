//! Canonical native billing response wire data; no network execution.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BillingCycle {
    pub year: i32,
    pub month: i32,
}

/// Cent value from the billing API (USD cents).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Cent {
    /// proto3 JSON omits zero-valued scalars, so a `$0` Cent arrives as `{}`; default to 0 rather than failing the whole parse.
    #[serde(default)]
    pub val: i64,
}

/// A usage period (weekly or monthly) from the newer credits config.
/// `start`/`end` are RFC 3339 timestamps.
/// `period_type` is the proto enum name (e.g. `USAGE_PERIOD_TYPE_WEEKLY`); it is kept so callers can distinguish weekly from monthly cycles.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UsagePeriod {
    #[serde(rename = "type", default, skip_serializing_if = "Option::is_none")]
    pub period_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end: Option<String>,
}

/// Usage summary for one past billing period.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BillingPeriodUsage {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub billing_cycle: Option<BillingCycle>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub included_used: Option<Cent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub on_demand_used: Option<Cent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_used: Option<Cent>,
}

/// Current billing configuration for Grok Build coding credits. Carries the newer credits-config fields (`credit_usage_percent`, `current_period`).
/// It also carries the deprecated `GrokBuildBillingConfig` fields (`monthly_limit`, `used`, `billing_period_*`). Consumers should prefer the new fields and fall back to the deprecated ones.
/// The same struct then works against both the new `GetGrokCreditsConfig` and the legacy `GetGrokBuildBillingConfig` responses.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BillingConfig {
    /// Included credit usage as a percentage of the allowance (0.0 to 100.0).
    /// Prefer it over deriving from `monthly_limit`/`used`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub credit_usage_percent: Option<f64>,
    /// Current usage period (weekly or monthly).
    /// Prefer it over `billing_period_start`/`billing_period_end`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_period: Option<UsagePeriod>,
    /// Deprecated: included monthly credit budget. Use `credit_usage_percent`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub monthly_limit: Option<Cent>,
    /// Deprecated: credits used this period. Use `credit_usage_percent`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub used: Option<Cent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub on_demand_cap: Option<Cent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub on_demand_used: Option<Cent>,
    /// Remaining prepaid (purchased) credit balance, positive: the "bought credits" the user has topped up.
    /// It comes from the credits config (`GetGrokCreditsConfig.prepaid_balance`) and is absent in the legacy billing shape.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prepaid_balance: Option<Cent>,
    /// Whether this user is on unified usage billing (a shared weekly/monthly pool).
    /// It comes from `GrokCreditsConfig.is_unified_billing_user`, which billing sets from the remote setting `unified_consumer_billing_enabled`.
    /// `None` when absent (legacy `GetGrokBuildBillingConfig` shape or older servers).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_unified_billing_user: Option<bool>,
    /// Deprecated: use `current_period.start`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub billing_period_start: Option<String>,
    /// Deprecated: use `current_period.end`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub billing_period_end: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub history: Vec<BillingPeriodUsage>,
}

/// Top-level response (primarily from `GET /rest/grok/credits` and the auto-topup-rule fetch).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BillingConfigResponse {
    pub config: Option<BillingConfig>,
    /// Whether on-demand credit usage is enabled; when `false`, the pager should hide on-demand controls.
    /// It comes from `RemoteSettings`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on_demand_enabled: Option<bool>,
    /// User-friendly subscription tier name (e.g. "SuperGrok Heavy").
    /// It comes from `RemoteSettings` so the pager can update its cached tier on every billing fetch without an extra request.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subscription_tier: Option<String>,
}

/// Auto top-up configuration (from GetAutoTopupRule).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AutoTopupRule {
    /// proto3 JSON omits `false`, so a disabled rule arrives without this field; default to `false` rather than failing the parse.
    /// A failed parse would keep a stale cached rule in the pager.
    #[serde(default)]
    pub enabled: bool,
    pub min_before_hitting_sl: Option<Cent>,
    pub topup_amount: Option<Cent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_amount_per_month: Option<Cent>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetAutoTopupRuleResponse {
    #[serde(default)]
    pub rule: Option<AutoTopupRule>,
}
