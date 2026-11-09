use std::collections::{HashMap, HashSet};

/// Tracks references delegated during one operation. Revocation cascades to
/// descendants and leaves a tombstone so a stale identity cannot be rebound.
#[derive(Clone, Debug, Default)]
pub struct DerivedReferenceGrants {
    parents: HashMap<String, String>,
    revoked: HashSet<String>,
}

impl DerivedReferenceGrants {
    /// Bind a child identity to its parent. Repeating the same binding is
    /// idempotent; aliases, cycles and revoked IDs are rejected.
    pub fn derive(&mut self, parent: &str, child: &str) -> bool {
        let existing_parent_matches = self.parents.get(child).map(|value| value == parent);
        let allowed = derivation_allowed(
            self.parents.contains_key(parent),
            self.is_active(parent),
            self.revoked.contains(child),
            parent == child,
            existing_parent_matches,
        );
        if !allowed {
            return false;
        }
        if existing_parent_matches.is_none() {
            self.parents.insert(child.to_string(), parent.to_string());
        }
        true
    }

    pub fn is_active(&self, reference: &str) -> bool {
        self.parents.contains_key(reference) && !self.revoked.contains(reference)
    }

    /// Revoke a delegated identity and every descendant derived from it.
    /// Direct operation selections are not represented here and remain owned
    /// by their separate binding lifetime.
    pub fn revoke(&mut self, reference: &str) -> bool {
        if !self.parents.contains_key(reference) {
            return false;
        }
        let mut pending = vec![reference.to_string()];
        while let Some(parent) = pending.pop() {
            if !self.revoked.insert(parent.clone()) {
                continue;
            }
            pending.extend(
                self.parents
                    .iter()
                    .filter(|(_, candidate_parent)| *candidate_parent == &parent)
                    .map(|(child, _)| child.clone()),
            );
        }
        true
    }
}

fn derivation_allowed(
    parent_is_derived: bool,
    parent_is_active: bool,
    child_is_revoked: bool,
    identities_match: bool,
    existing_parent_matches: Option<bool>,
) -> bool {
    !identities_match
        && !child_is_revoked
        && (!parent_is_derived || parent_is_active)
        && existing_parent_matches.unwrap_or(true)
}

#[cfg(test)]
mod tests {
    use super::DerivedReferenceGrants;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn revoking_a_parent_invalidates_every_generated_descendant(
            depth in 1usize..32,
            sibling_count in 0usize..16,
        ) {
            let mut grants = DerivedReferenceGrants::default();
            prop_assert!(grants.derive("root", "directory"));
            let descendants = (0..depth)
                .map(|index| format!("child-{index}"))
                .collect::<Vec<_>>();
            for child in &descendants {
                prop_assert!(grants.derive("directory", child));
            }
            let siblings = (0..sibling_count)
                .map(|index| format!("sibling-{index}"))
                .collect::<Vec<_>>();
            for child in &siblings {
                prop_assert!(grants.derive("root", child));
            }

            prop_assert!(grants.revoke("directory"));
            prop_assert!(!grants.is_active("directory"));
            for child in descendants {
                prop_assert!(!grants.is_active(&child));
            }
            for child in siblings {
                prop_assert!(grants.is_active(&child));
            }
            prop_assert!(!grants.derive("root", "directory"));
        }

        #[test]
        fn an_existing_identity_cannot_be_reparented(
            original_parent in "parent-[a-z]{1,8}",
            requested_parent in "parent-[a-z]{1,8}",
        ) {
            prop_assume!(original_parent != requested_parent);
            let mut grants = DerivedReferenceGrants::default();
            prop_assert!(grants.derive(&original_parent, "child"));
            prop_assert!(!grants.derive(&requested_parent, "child"));
        }
    }

    #[test]
    fn revoking_a_derived_directory_cascades_to_nested_resources() {
        let mut grants = DerivedReferenceGrants::default();
        assert!(grants.derive("root", "directory"));
        assert!(grants.derive("directory", "nested-file"));
        assert!(grants.derive("root", "sibling"));
        assert!(grants.revoke("directory"));
        assert!(!grants.is_active("directory"));
        assert!(!grants.is_active("nested-file"));
        assert!(grants.is_active("sibling"));
        assert!(!grants.derive("nested-file", "escaped"));
    }
}

#[cfg(kani)]
mod verification {
    use super::derivation_allowed;

    #[kani::proof]
    fn stale_or_invalid_derivations_never_grant_authority() {
        let parent_is_derived: bool = kani::any();
        let parent_is_active: bool = kani::any();
        let child_is_revoked: bool = kani::any();
        let identities_match: bool = kani::any();
        let existing_parent_matches: Option<bool> = kani::any();

        let allowed = derivation_allowed(
            parent_is_derived,
            parent_is_active,
            child_is_revoked,
            identities_match,
            existing_parent_matches,
        );
        if allowed {
            assert!(!child_is_revoked);
            assert!(!identities_match);
            assert!(!parent_is_derived || parent_is_active);
            assert!(existing_parent_matches.unwrap_or(true));
        }
    }
}
