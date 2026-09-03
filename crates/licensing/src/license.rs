//! Offline license keys.
//!
//! A key is a signed envelope the buyer pastes into Settings. Shelf checks the
//! signature locally, so activation works without a server, without an account
//! and without sending anything anywhere.

use crate::envelope::{self, Error};
use ed25519_dalek::VerifyingKey;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum Tier {
    #[default]
    Free,
    Pro,
    /// A tier minted by a newer version of the tool. Treated as Free so an old
    /// build never hands out access it cannot reason about.
    #[serde(other)]
    Unknown,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct License {
    /// Order reference, so Louis can match a key to a payment in support mail.
    pub id: String,
    /// Shown in Settings. Personalised on purpose: a key with someone else's
    /// name on it is uncomfortable to pass around.
    pub name: String,
    pub tier: Tier,
    /// Unix seconds.
    pub issued: i64,
    /// Unix seconds. `None` means it never runs out.
    pub expires: Option<i64>,
}

impl License {
    pub fn expired_at(&self, now: i64) -> bool {
        self.expires.is_some_and(|expires| now >= expires)
    }

    pub fn tier_at(&self, now: i64) -> Tier {
        if self.expired_at(now) {
            Tier::Free
        } else {
            self.tier
        }
    }
}

pub fn verify(key_blob: &str, public_key: &VerifyingKey) -> Result<License, Error> {
    envelope::open(key_blob, public_key)
}
