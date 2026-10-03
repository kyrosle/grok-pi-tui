use super::notification::PromptUsage;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionUsageResponse {
    pub usage: PromptUsage,
}
