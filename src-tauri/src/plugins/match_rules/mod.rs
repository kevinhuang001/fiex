//! Manifest `match` rules shared by Info, Preview, Decorations and ContextMenu.
//!
//! `match` is an inclusion predicate: Core only invokes the provider when the
//! resource (or directory/selection) satisfies it. Missing or empty rules match
//! everything.
use fiex_provider_contracts::MatchResource;
pub use fiex_resource::mime_for;
use serde_json::Value;
use std::ops::Deref;

mod location;
pub(crate) use location::typed_locations_for_snapshot;
pub use location::{directory_match_resource, typed_locations_for};

/// Core uses the SDK's one authoritative manifest predicate and DTO.
/// This thin adapter only supplies a trusted directory snapshot for blank
/// canvas events; it never defines independent match/selection semantics.
#[derive(Clone, Debug, Default, serde::Deserialize)]
#[serde(transparent)]
pub struct MatchRules(pub fiex_provider_contracts::ProviderMatchRule);

impl Deref for MatchRules {
    type Target = fiex_provider_contracts::ProviderMatchRule;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl MatchRules {
    pub fn from_contribution(contribution: &Value) -> Result<Self, serde_json::Error> {
        contribution
            .get("match")
            .map(|value| serde_json::from_value(value.clone()))
            .unwrap_or_else(|| Ok(Self::default()))
    }

    pub fn matches_resources(&self, targets: &[MatchResource], directory: &str) -> bool {
        let blank = (targets.is_empty() && !directory.is_empty())
            .then(|| location::directory_match_resource(directory))
            .flatten();
        fiex_provider_contracts::ProviderMatcher::matches(
            &self.0,
            targets,
            blank.as_ref(),
            targets.len(),
        )
    }
}

pub fn validate(
    value: Option<&Value>,
    _allow_contains: bool,
    allow_selection: bool,
) -> Result<(), String> {
    fiex_provider_contracts::ProviderMatchRule::validate(value, allow_selection)
}

#[cfg(test)]
mod tests;
