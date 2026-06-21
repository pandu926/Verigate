//! Case lifecycle state machine with explicit transition table.
//!
//! All valid state transitions are defined here. No other code path
//! may update case status without going through `validate_transition`.

use crate::domain::types::CaseStatus;
use crate::error::AppError;

/// Allowed transitions as a static lookup table.
/// Format: (from_state, to_state)
const TRANSITIONS: &[(CaseStatus, CaseStatus)] = &[
    (CaseStatus::Created, CaseStatus::Discovery),
    (CaseStatus::Discovery, CaseStatus::Collecting),
    (CaseStatus::Collecting, CaseStatus::Verifying),
    (CaseStatus::Verifying, CaseStatus::Assessing),
    (CaseStatus::Assessing, CaseStatus::Review),
    (CaseStatus::Assessing, CaseStatus::Approved),
    (CaseStatus::Review, CaseStatus::Approved),
    (CaseStatus::Review, CaseStatus::Blocked),
    (CaseStatus::Blocked, CaseStatus::Discovery),
    // Any non-Approved state can transition to Blocked (handled in is_valid_transition)
];

/// Check whether a transition is valid according to the state machine rules.
fn is_valid_transition(from: &CaseStatus, to: &CaseStatus) -> bool {
    // Approved is a terminal state — no transitions out
    if *from == CaseStatus::Approved {
        return false;
    }

    // Any non-Approved state can transition to Blocked
    if *to == CaseStatus::Blocked && *from != CaseStatus::Approved {
        return true;
    }

    // Check explicit transition table
    TRANSITIONS
        .iter()
        .any(|(f, t)| f == from && t == to)
}

/// Validate a state transition. Returns `Ok(())` if valid, or an
/// `AppError::InvalidTransition` with the current state and allowed targets.
pub fn validate_transition(from: &CaseStatus, to: &CaseStatus) -> Result<(), AppError> {
    if is_valid_transition(from, to) {
        Ok(())
    } else {
        let allowed = allowed_transitions(from);
        Err(AppError::InvalidTransition {
            current_state: format!("{:?}", from),
            allowed: allowed.iter().map(|s| format!("{:?}", s)).collect(),
        })
    }
}

/// Return all valid target states from a given state.
pub fn allowed_transitions(from: &CaseStatus) -> Vec<CaseStatus> {
    if *from == CaseStatus::Approved {
        return Vec::new();
    }

    let mut targets: Vec<CaseStatus> = TRANSITIONS
        .iter()
        .filter(|(f, _)| f == from)
        .map(|(_, t)| t.clone())
        .collect();

    // Add Blocked for any non-Approved state if not already present
    if *from != CaseStatus::Approved
        && *from != CaseStatus::Blocked
        && !targets.contains(&CaseStatus::Blocked)
    {
        targets.push(CaseStatus::Blocked);
    }

    targets
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_transition_created_to_discovery() {
        assert!(validate_transition(&CaseStatus::Created, &CaseStatus::Discovery).is_ok());
    }

    #[test]
    fn valid_transition_discovery_to_collecting() {
        assert!(validate_transition(&CaseStatus::Discovery, &CaseStatus::Collecting).is_ok());
    }

    #[test]
    fn valid_transition_collecting_to_verifying() {
        assert!(validate_transition(&CaseStatus::Collecting, &CaseStatus::Verifying).is_ok());
    }

    #[test]
    fn valid_transition_verifying_to_assessing() {
        assert!(validate_transition(&CaseStatus::Verifying, &CaseStatus::Assessing).is_ok());
    }

    #[test]
    fn valid_transition_assessing_to_review() {
        assert!(validate_transition(&CaseStatus::Assessing, &CaseStatus::Review).is_ok());
    }

    #[test]
    fn valid_transition_review_to_approved() {
        assert!(validate_transition(&CaseStatus::Review, &CaseStatus::Approved).is_ok());
    }

    #[test]
    fn valid_transition_review_to_blocked() {
        assert!(validate_transition(&CaseStatus::Review, &CaseStatus::Blocked).is_ok());
    }

    #[test]
    fn valid_transition_blocked_to_discovery() {
        assert!(validate_transition(&CaseStatus::Blocked, &CaseStatus::Discovery).is_ok());
    }

    #[test]
    fn invalid_transition_created_to_approved() {
        let result = validate_transition(&CaseStatus::Created, &CaseStatus::Approved);
        assert!(result.is_err());
    }

    #[test]
    fn invalid_transition_approved_to_anything() {
        assert!(validate_transition(&CaseStatus::Approved, &CaseStatus::Discovery).is_err());
        assert!(validate_transition(&CaseStatus::Approved, &CaseStatus::Blocked).is_err());
        assert!(validate_transition(&CaseStatus::Approved, &CaseStatus::Created).is_err());
    }

    #[test]
    fn invalid_transition_collecting_to_review() {
        assert!(validate_transition(&CaseStatus::Collecting, &CaseStatus::Review).is_err());
    }

    #[test]
    fn any_non_approved_state_can_transition_to_blocked() {
        let states = [
            CaseStatus::Created,
            CaseStatus::Discovery,
            CaseStatus::Collecting,
            CaseStatus::Verifying,
            CaseStatus::Assessing,
            CaseStatus::Review,
        ];
        for state in &states {
            assert!(
                validate_transition(state, &CaseStatus::Blocked).is_ok(),
                "Expected {:?} -> Blocked to be valid",
                state
            );
        }
    }

    #[test]
    fn blocked_can_only_go_to_discovery() {
        assert!(validate_transition(&CaseStatus::Blocked, &CaseStatus::Discovery).is_ok());
        assert!(validate_transition(&CaseStatus::Blocked, &CaseStatus::Collecting).is_err());
        assert!(validate_transition(&CaseStatus::Blocked, &CaseStatus::Verifying).is_err());
        assert!(validate_transition(&CaseStatus::Blocked, &CaseStatus::Assessing).is_err());
        assert!(validate_transition(&CaseStatus::Blocked, &CaseStatus::Review).is_err());
        assert!(validate_transition(&CaseStatus::Blocked, &CaseStatus::Approved).is_err());
    }

    #[test]
    fn allowed_transitions_from_created() {
        let allowed = allowed_transitions(&CaseStatus::Created);
        assert!(allowed.contains(&CaseStatus::Discovery));
        assert!(allowed.contains(&CaseStatus::Blocked));
        assert_eq!(allowed.len(), 2);
    }

    #[test]
    fn allowed_transitions_from_review() {
        let allowed = allowed_transitions(&CaseStatus::Review);
        assert!(allowed.contains(&CaseStatus::Approved));
        assert!(allowed.contains(&CaseStatus::Blocked));
        assert_eq!(allowed.len(), 2);
    }

    #[test]
    fn allowed_transitions_from_approved_is_empty() {
        let allowed = allowed_transitions(&CaseStatus::Approved);
        assert!(allowed.is_empty());
    }

    #[test]
    fn allowed_transitions_from_blocked() {
        let allowed = allowed_transitions(&CaseStatus::Blocked);
        assert!(allowed.contains(&CaseStatus::Discovery));
        assert_eq!(allowed.len(), 1);
    }
}
