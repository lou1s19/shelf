//! The signed file on the website that says which version is the floor and
//! which features cost money.
//!
//! It is signed for the same reason the update feed is: without a signature,
//! anyone able to answer for the domain (a captive portal, a company proxy)
//! could hand Shelf a policy of their choosing.

use crate::envelope::{self, Error};
use crate::feature::Feature;
use ed25519_dalek::VerifyingKey;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Policy {
    /// Unix seconds. Only ever replaced by a policy with a newer stamp, so a
    /// stale copy cannot be replayed to undo a raised floor.
    pub issued: i64,
    /// Anything below this is refused. `0.0.0` blocks nobody.
    pub minimum_version: String,
    /// Days of warning before the floor actually bites, counted from `issued`.
    #[serde(default)]
    pub grace_days: u32,
    /// Feature keys that require a license. Unknown keys are ignored.
    #[serde(default)]
    pub paid_features: Vec<String>,
    #[serde(default)]
    pub download_url: Option<String>,
    #[serde(default)]
    pub buy_url: Option<String>,
    /// One line shown on the update wall, for a reason the version number
    /// cannot carry on its own.
    #[serde(default)]
    pub message: Option<String>,
}

impl Default for Policy {
    /// What Shelf assumes before it has ever reached the network: no floor,
    /// nothing paid.
    fn default() -> Self {
        Self {
            issued: 0,
            minimum_version: "0.0.0".to_string(),
            grace_days: 0,
            paid_features: Vec::new(),
            download_url: None,
            buy_url: None,
            message: None,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase", tag = "type")]
pub enum Verdict {
    Ok,
    /// Below the floor, but still inside the grace window.
    UpdateSoon {
        minimum: String,
        deadline: i64,
    },
    /// Below the floor and out of grace. The app blocks here.
    UpdateRequired {
        minimum: String,
    },
}

impl Policy {
    pub fn verify(blob: &str, public_key: &VerifyingKey) -> Result<Policy, Error> {
        envelope::open(blob, public_key)
    }

    pub fn paid(&self) -> BTreeSet<Feature> {
        self.paid_features
            .iter()
            .filter_map(|key| Feature::from_key(key))
            .collect()
    }

    /// Unknown feature keys, kept so the log can say why nothing happened.
    pub fn unknown_features(&self) -> Vec<&str> {
        self.paid_features
            .iter()
            .filter(|key| Feature::from_key(key).is_none())
            .map(String::as_str)
            .collect()
    }

    pub fn verdict(&self, current_version: &str, now: i64) -> Verdict {
        let (Ok(current), Ok(minimum)) = (
            semver::Version::parse(current_version),
            semver::Version::parse(&self.minimum_version),
        ) else {
            // An unparseable version on either side is our own mistake, not the
            // user's. Locking them out over it would be the worse failure.
            return Verdict::Ok;
        };

        if current >= minimum {
            return Verdict::Ok;
        }

        let deadline = self
            .issued
            .saturating_add(i64::from(self.grace_days) * 86_400);
        if now < deadline {
            Verdict::UpdateSoon {
                minimum: self.minimum_version.clone(),
                deadline,
            }
        } else {
            Verdict::UpdateRequired {
                minimum: self.minimum_version.clone(),
            }
        }
    }

    /// Takes `other` only if it is not older than what we already trust, so an
    /// older copy replayed from a cache or a stale CDN edge cannot lower a
    /// floor that was already raised. Says whether it took it.
    pub fn replace_if_newer(&mut self, other: Policy) -> bool {
        if other.issued < self.issued {
            return false;
        }
        *self = other;
        true
    }
}
