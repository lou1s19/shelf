//! Shelf's update floor and payment gate.
//!
//! Two independent pieces, both built on one signed-envelope format:
//!
//! * a **policy** fetched from the website, saying which version is the minimum
//!   and which features cost money,
//! * a **license key** the buyer pastes in, checked offline.
//!
//! Both are inert until Louis signs a policy that says otherwise. With the
//! default policy every feature is free and no version is refused.
//!
//! Worth being honest about the limits: Shelf is AGPLv3, so anyone who receives
//! it also receives the source and may compile the gate away. This is a lock on
//! the front door, not a vault. It exists to make paying the easy path.

pub mod envelope;
pub mod feature;
pub mod license;
pub mod policy;

pub use envelope::{Error, parse_public_key};
pub use feature::Feature;
pub use license::{License, Tier};
pub use policy::{Policy, Verdict};

use std::collections::BTreeSet;

/// What the running copy is allowed to do, once policy and license are folded
/// together.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Entitlements {
    pub tier: Tier,
    pub paid: BTreeSet<Feature>,
}

impl Entitlements {
    pub fn new(tier: Tier, paid: BTreeSet<Feature>) -> Self {
        Self { tier, paid }
    }

    pub fn allows(&self, feature: Feature) -> bool {
        if self.tier == Tier::Pro {
            return true;
        }
        !self.paid.contains(&feature) && !self.paid.contains(&Feature::App)
    }

    /// The features this copy currently cannot reach.
    pub fn locked(&self) -> Vec<Feature> {
        Feature::ALL
            .iter()
            .copied()
            .filter(|f| !self.allows(*f))
            .collect()
    }
}

pub fn now_unix() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn policy_with(minimum: &str, grace_days: u32, issued: i64) -> Policy {
        Policy {
            issued,
            minimum_version: minimum.to_string(),
            grace_days,
            ..Policy::default()
        }
    }

    #[test]
    fn default_policy_blocks_nobody() {
        assert_eq!(Policy::default().verdict("0.5.9", 0), Verdict::Ok);
        assert!(Policy::default().paid().is_empty());
    }

    #[test]
    fn version_below_the_floor_is_refused() {
        let policy = policy_with("1.0.0", 0, 1_000);
        assert_eq!(
            policy.verdict("0.5.9", 1_000),
            Verdict::UpdateRequired {
                minimum: "1.0.0".into()
            }
        );
        assert_eq!(policy.verdict("1.0.0", 1_000), Verdict::Ok);
        assert_eq!(policy.verdict("1.2.0", 1_000), Verdict::Ok);
    }

    #[test]
    fn grace_window_warns_first_then_blocks() {
        let policy = policy_with("1.0.0", 7, 1_000);
        let deadline = 1_000 + 7 * 86_400;
        assert_eq!(
            policy.verdict("0.5.9", 1_000),
            Verdict::UpdateSoon {
                minimum: "1.0.0".into(),
                deadline
            }
        );
        assert_eq!(
            policy.verdict("0.5.9", deadline),
            Verdict::UpdateRequired {
                minimum: "1.0.0".into()
            }
        );
    }

    #[test]
    fn an_unreadable_version_never_locks_the_user_out() {
        let policy = policy_with("not-a-version", 0, 0);
        assert_eq!(policy.verdict("0.5.9", 0), Verdict::Ok);
        assert_eq!(
            policy_with("1.0.0", 0, 0).verdict("nightly", 0),
            Verdict::Ok
        );
    }

    #[test]
    fn unknown_feature_keys_are_ignored_not_fatal() {
        let policy = Policy {
            paid_features: vec!["ocr".into(), "time-machine".into()],
            ..Policy::default()
        };
        assert_eq!(policy.paid(), BTreeSet::from([Feature::Ocr]));
        assert_eq!(policy.unknown_features(), vec!["time-machine"]);
    }

    #[test]
    fn a_stale_policy_cannot_lower_the_floor() {
        let mut held = policy_with("1.0.0", 0, 2_000);
        assert!(!held.replace_if_newer(policy_with("0.0.0", 0, 1_000)));
        assert_eq!(held.minimum_version, "1.0.0");
    }

    #[test]
    fn a_newer_policy_is_taken_even_when_it_lowers_the_floor() {
        // Louis has to be able to take a floor back if he set it by mistake.
        let mut held = policy_with("1.0.0", 0, 1_000);
        assert!(held.replace_if_newer(policy_with("0.0.0", 0, 2_000)));
        assert_eq!(held.minimum_version, "0.0.0");
    }

    #[test]
    fn a_policy_reissued_in_the_same_second_still_applies() {
        let mut held = policy_with("1.0.0", 0, 1_000);
        assert!(held.replace_if_newer(policy_with("1.2.0", 0, 1_000)));
        assert_eq!(held.minimum_version, "1.2.0");
    }

    #[test]
    fn free_tier_keeps_everything_that_is_not_listed() {
        let e = Entitlements::new(Tier::Free, BTreeSet::from([Feature::Ocr]));
        assert!(!e.allows(Feature::Ocr));
        assert!(e.allows(Feature::Screenshot));
        assert_eq!(e.locked(), vec![Feature::Ocr]);
    }

    #[test]
    fn listing_the_app_locks_every_feature() {
        let e = Entitlements::new(Tier::Free, BTreeSet::from([Feature::App]));
        assert!(!e.allows(Feature::Screenshot));
        assert!(!e.allows(Feature::Export));
    }

    #[test]
    fn a_pro_license_opens_everything() {
        let e = Entitlements::new(Tier::Pro, BTreeSet::from([Feature::App]));
        assert!(e.allows(Feature::Screenshot));
        assert!(e.locked().is_empty());
    }

    #[test]
    fn an_unknown_tier_is_treated_as_free() {
        let e = Entitlements::new(Tier::Unknown, BTreeSet::from([Feature::Ocr]));
        assert!(!e.allows(Feature::Ocr));
    }

    #[test]
    fn an_expired_license_falls_back_to_free() {
        let l = License {
            id: "order-1".into(),
            name: "Louis".into(),
            tier: Tier::Pro,
            issued: 0,
            expires: Some(1_000),
        };
        assert_eq!(l.tier_at(999), Tier::Pro);
        assert_eq!(l.tier_at(1_000), Tier::Free);
    }

    #[test]
    fn a_license_without_an_end_date_never_expires() {
        let l = License {
            id: "order-2".into(),
            name: "Louis".into(),
            tier: Tier::Pro,
            issued: 0,
            expires: None,
        };
        assert_eq!(l.tier_at(i64::MAX), Tier::Pro);
    }
}

#[cfg(all(test, feature = "mint"))]
mod signature_tests {
    use super::*;
    use ed25519_dalek::SigningKey;
    use rand_core::OsRng;

    fn key() -> SigningKey {
        SigningKey::generate(&mut OsRng)
    }

    #[test]
    fn a_minted_license_verifies_with_the_matching_public_key() {
        let signing = key();
        let license = License {
            id: "1001".into(),
            name: "Jane Doe".into(),
            tier: Tier::Pro,
            issued: 10,
            expires: None,
        };
        let blob = envelope::seal(&license, &signing).unwrap();
        let back = license::verify(&blob, &signing.verifying_key()).unwrap();
        assert_eq!(back.id, "1001");
        assert_eq!(back.tier, Tier::Pro);
    }

    #[test]
    fn a_key_from_a_different_signer_is_refused() {
        let blob = envelope::seal(
            &License {
                id: "forged".into(),
                name: "Nobody".into(),
                tier: Tier::Pro,
                issued: 0,
                expires: None,
            },
            &key(),
        )
        .unwrap();
        assert!(matches!(
            license::verify(&blob, &key().verifying_key()),
            Err(Error::BadSignature)
        ));
    }

    #[test]
    fn a_tampered_payload_is_refused() {
        let signing = key();
        let blob = envelope::seal(
            &Policy {
                minimum_version: "1.0.0".into(),
                ..Policy::default()
            },
            &signing,
        )
        .unwrap();

        let mut parts: Vec<&str> = blob.split('.').collect();
        let swapped = envelope::seal(
            &Policy {
                minimum_version: "9.9.9".into(),
                ..Policy::default()
            },
            &key(),
        )
        .unwrap();
        let other_payload = swapped.split('.').nth(1).unwrap();
        parts[1] = other_payload;

        assert!(matches!(
            Policy::verify(&parts.join("."), &signing.verifying_key()),
            Err(Error::BadSignature)
        ));
    }

    #[test]
    fn garbage_is_rejected_without_panicking() {
        let public = key().verifying_key();
        for junk in [
            "",
            "nonsense",
            "SHELF1.only-two",
            "SHELF2.a.b",
            "SHELF1.!.!",
        ] {
            assert!(license::verify(junk, &public).is_err(), "accepted {junk:?}");
        }
    }

    #[test]
    fn the_public_key_round_trips_through_hex() {
        let signing = key();
        let hex = envelope::encode_hex(signing.verifying_key().as_bytes());
        assert_eq!(parse_public_key(&hex).unwrap(), signing.verifying_key());
        assert!(parse_public_key("zz").is_err());
        assert!(parse_public_key("00").is_err());
    }
}
