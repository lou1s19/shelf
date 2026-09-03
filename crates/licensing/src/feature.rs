//! The things a future release could put behind a payment.
//!
//! The list lives here rather than in the policy file so that a build only ever
//! gates what it actually understands. A policy naming a feature this build has
//! never heard of is ignored on purpose: the minimum-version gate is what pulls
//! everyone onto a build that knows the newer names.

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(rename_all = "kebab-case")]
pub enum Feature {
    /// The whole app. Listing this makes Shelf paid outright.
    App,
    StudioRecording,
    InstantRecording,
    Screenshot,
    ScreenshotEditor,
    Teleprompter,
    Captions,
    Ocr,
    Export,
}

impl Feature {
    pub const ALL: &'static [Feature] = &[
        Feature::App,
        Feature::StudioRecording,
        Feature::InstantRecording,
        Feature::Screenshot,
        Feature::ScreenshotEditor,
        Feature::Teleprompter,
        Feature::Captions,
        Feature::Ocr,
        Feature::Export,
    ];

    pub fn key(self) -> &'static str {
        match self {
            Feature::App => "app",
            Feature::StudioRecording => "studio-recording",
            Feature::InstantRecording => "instant-recording",
            Feature::Screenshot => "screenshot",
            Feature::ScreenshotEditor => "screenshot-editor",
            Feature::Teleprompter => "teleprompter",
            Feature::Captions => "captions",
            Feature::Ocr => "ocr",
            Feature::Export => "export",
        }
    }

    /// What the user is told when the feature is locked.
    pub fn label(self) -> &'static str {
        match self {
            Feature::App => "Shelf",
            Feature::StudioRecording => "Studio recording",
            Feature::InstantRecording => "Instant recording",
            Feature::Screenshot => "Screenshots",
            Feature::ScreenshotEditor => "The screenshot editor",
            Feature::Teleprompter => "The teleprompter",
            Feature::Captions => "Captions",
            Feature::Ocr => "Text recognition",
            Feature::Export => "Exporting",
        }
    }

    pub fn from_key(key: &str) -> Option<Feature> {
        Feature::ALL.iter().copied().find(|f| f.key() == key)
    }
}
