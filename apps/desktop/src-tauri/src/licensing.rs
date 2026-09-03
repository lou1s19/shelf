//! Ties the signed policy and the offline license key into the running app.
//!
//! Nothing here restricts anything until a signed policy says so. Shipping it
//! now means a later decision to charge for Shelf, or for parts of it, is a
//! signed file on the website plus a release, instead of a change every
//! installed copy would have to opt into.
//!
//! During development, set `SHELF_POLICY_URL=` (empty) to keep the app off the
//! network entirely.

use crate::windows::ShowCapWindow;
use serde::{Deserialize, Serialize};
use shelf_licensing::{Entitlements, Feature, License, Policy, Tier, Verdict, now_unix};
use specta::Type;
use std::sync::RwLock;
use std::time::Duration;
use tauri::{AppHandle, Manager, Wry};
use tauri_plugin_store::StoreExt;
use tracing::{debug, info, warn};

/// The public half of the key in `~/.shelf-licensing/secret.key`. Replacing it
/// invalidates every key and policy signed with the old secret.
const PUBLIC_KEY: &str = "4c078206f5704f0520fd8ade6749c43b63b73e14cbf6617354597cf060ae7def";

/// Where the signed policy lives. Empty disables the check.
const DEFAULT_POLICY_URL: &str = "https://shelf-website-mu.vercel.app/policy.txt";

const STORE_KEY: &str = "licensing";
const REFRESH_INTERVAL: Duration = Duration::from_secs(6 * 60 * 60);

fn policy_url() -> Option<String> {
    let url = std::env::var("SHELF_POLICY_URL")
        .ok()
        .unwrap_or_else(|| DEFAULT_POLICY_URL.to_string());
    let url = url.trim().to_string();
    (!url.is_empty()).then_some(url)
}

/// Kept as the signed blobs rather than the decoded values: a blob that no
/// longer verifies (key rotated, file edited by hand) is simply dropped on the
/// next start instead of being trusted because it once was.
#[derive(Serialize, Deserialize, Default, Debug, Clone)]
#[serde(rename_all = "camelCase")]
struct Stored {
    #[serde(default)]
    policy: Option<String>,
    #[serde(default)]
    license: Option<String>,
    #[serde(default)]
    last_checked: Option<i64>,
}

impl Stored {
    fn load(app: &AppHandle<Wry>) -> Self {
        app.store("store")
            .ok()
            .and_then(|store| store.get(STORE_KEY))
            .and_then(|value| serde_json::from_value(value).ok())
            .unwrap_or_default()
    }

    fn save(&self, app: &AppHandle<Wry>) {
        let Ok(store) = app.store("store") else {
            warn!("licensing: no store to save into");
            return;
        };
        match serde_json::to_value(self) {
            Ok(value) => {
                store.set(STORE_KEY, value);
                if let Err(e) = store.save() {
                    warn!("licensing: could not save: {e}");
                }
            }
            Err(e) => warn!("licensing: could not serialize: {e}"),
        }
    }
}

#[derive(Serialize, Type, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase", tag = "type")]
pub enum UpdateState {
    Ok,
    /// Below the floor, still inside the grace window. Seconds, as f64: specta
    /// refuses i64 because JavaScript would need a BigInt for it, and a unix
    /// timestamp is nowhere near the point where f64 loses whole seconds.
    UpdateSoon {
        minimum: String,
        deadline: f64,
    },
    /// Below the floor and out of grace. Everything is refused.
    UpdateRequired {
        minimum: String,
    },
}

impl From<Verdict> for UpdateState {
    fn from(verdict: Verdict) -> Self {
        match verdict {
            Verdict::Ok => UpdateState::Ok,
            Verdict::UpdateSoon { minimum, deadline } => UpdateState::UpdateSoon {
                minimum,
                deadline: deadline as f64,
            },
            Verdict::UpdateRequired { minimum } => UpdateState::UpdateRequired { minimum },
        }
    }
}

#[derive(Serialize, Type, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct LicensingStatus {
    pub tier: String,
    pub licensed_to: Option<String>,
    pub license_id: Option<String>,
    pub license_expires: Option<f64>,
    /// Feature keys this copy cannot reach right now. Empty in a free build.
    pub locked_features: Vec<String>,
    pub update: UpdateState,
    pub current_version: String,
    pub buy_url: Option<String>,
    pub download_url: Option<String>,
    pub message: Option<String>,
    pub last_checked: Option<f64>,
    /// False when no policy URL is configured, so Settings can say so instead
    /// of showing a check that never runs.
    pub checks_enabled: bool,
}

#[derive(Serialize, Type, tauri_specta::Event, Clone, Debug)]
pub struct LicensingChanged(pub LicensingStatus);

#[derive(Default)]
pub struct LicensingState {
    inner: RwLock<Inner>,
}

#[derive(Default)]
struct Inner {
    policy: Policy,
    license: Option<License>,
    last_checked: Option<i64>,
}

impl LicensingState {
    fn entitlements(&self, now: i64) -> Entitlements {
        let inner = self.inner.read().unwrap();
        let tier = inner
            .license
            .as_ref()
            .map_or(Tier::Free, |license| license.tier_at(now));
        Entitlements::new(tier, inner.policy.paid())
    }

    fn update_state(&self, now: i64) -> UpdateState {
        self.inner
            .read()
            .unwrap()
            .policy
            .verdict(current_version(), now)
            .into()
    }

    fn status(&self, now: i64) -> LicensingStatus {
        let entitlements = self.entitlements(now);
        let inner = self.inner.read().unwrap();
        LicensingStatus {
            tier: match entitlements.tier {
                Tier::Pro => "pro",
                _ => "free",
            }
            .to_string(),
            licensed_to: inner.license.as_ref().map(|l| l.name.clone()),
            license_id: inner.license.as_ref().map(|l| l.id.clone()),
            license_expires: inner
                .license
                .as_ref()
                .and_then(|l| l.expires)
                .map(|e| e as f64),
            locked_features: entitlements
                .locked()
                .into_iter()
                .map(|f| f.key().to_string())
                .collect(),
            update: inner.policy.verdict(current_version(), now).into(),
            current_version: current_version().to_string(),
            buy_url: inner.policy.buy_url.clone(),
            download_url: inner.policy.download_url.clone(),
            message: inner.policy.message.clone(),
            last_checked: inner.last_checked.map(|t| t as f64),
            checks_enabled: policy_url().is_some(),
        }
    }
}

pub fn current_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

fn public_key() -> Option<ed25519_dalek::VerifyingKey> {
    match shelf_licensing::parse_public_key(PUBLIC_KEY) {
        Ok(key) => Some(key),
        Err(e) => {
            // Only reachable if the constant above was mistyped. Refusing every
            // signature would lock out paying users over our own typo, so the
            // gate stays open instead.
            warn!("licensing: public key unusable, gate disabled: {e}");
            None
        }
    }
}

/// The one place a feature asks whether it may run.
pub fn require(app: &AppHandle<Wry>, feature: Feature) -> Result<(), String> {
    let state = app.state::<LicensingState>();
    let now = now_unix();

    if let UpdateState::UpdateRequired { minimum } = state.update_state(now) {
        let app = app.clone();
        tauri::async_runtime::spawn(async move {
            let _ = ShowCapWindow::UpdateRequired.show(&app).await;
        });
        return Err(format!(
            "Shelf {minimum} is required before you can keep using it."
        ));
    }

    if state.entitlements(now).allows(feature) {
        return Ok(());
    }

    Err(format!("{} needs a Shelf license.", feature.label()))
}

pub fn status(app: &AppHandle<Wry>) -> LicensingStatus {
    app.state::<LicensingState>().status(now_unix())
}

fn broadcast(app: &AppHandle<Wry>) {
    use tauri_specta::Event;
    let status = status(app);
    if let Err(e) = LicensingChanged(status).emit(app) {
        warn!("licensing: could not announce the change: {e}");
    }
}

#[tauri::command(async)]
#[specta::specta]
pub fn licensing_status(app: AppHandle<Wry>) -> LicensingStatus {
    status(&app)
}

#[tauri::command(async)]
#[specta::specta]
pub fn licensing_activate(app: AppHandle<Wry>, key: String) -> Result<LicensingStatus, String> {
    let public = public_key().ok_or("Shelf cannot check licenses in this build.")?;
    let key = key.trim().to_string();

    let license = shelf_licensing::license::verify(&key, &public)
        .map_err(|_| "That is not a valid Shelf license key.".to_string())?;

    if license.expired_at(now_unix()) {
        return Err("That license has run out.".to_string());
    }

    {
        let state = app.state::<LicensingState>();
        state.inner.write().unwrap().license = Some(license);
    }

    let mut stored = Stored::load(&app);
    stored.license = Some(key);
    stored.save(&app);

    info!("licensing: license activated");
    broadcast(&app);
    Ok(status(&app))
}

#[tauri::command(async)]
#[specta::specta]
pub fn licensing_deactivate(app: AppHandle<Wry>) -> LicensingStatus {
    app.state::<LicensingState>().inner.write().unwrap().license = None;

    let mut stored = Stored::load(&app);
    stored.license = None;
    stored.save(&app);

    broadcast(&app);
    status(&app)
}

#[tauri::command(async)]
#[specta::specta]
pub async fn licensing_refresh(app: AppHandle<Wry>) -> Result<LicensingStatus, String> {
    refresh_policy(&app).await?;
    Ok(status(&app))
}

/// Reads whatever was cached last time. Runs before the first network call so
/// the app knows where it stands even offline.
pub fn init(app: &AppHandle<Wry>) {
    let stored = Stored::load(app);
    let Some(public) = public_key() else {
        return;
    };

    let state = app.state::<LicensingState>();
    let mut inner = state.inner.write().unwrap();
    inner.last_checked = stored.last_checked;

    if let Some(blob) = &stored.policy {
        match Policy::verify(blob, &public) {
            Ok(policy) => inner.policy = policy,
            Err(e) => warn!("licensing: cached policy no longer verifies, ignoring it: {e}"),
        }
    }

    if let Some(blob) = &stored.license {
        match shelf_licensing::license::verify(blob, &public) {
            Ok(license) => inner.license = Some(license),
            Err(e) => warn!("licensing: stored license no longer verifies, ignoring it: {e}"),
        }
    }
}

/// Fetches the policy, keeps it only if it is signed and newer than the one we
/// already trust. Any failure leaves the cached policy in place: a flaky
/// connection must never turn into a locked app.
async fn refresh_policy(app: &AppHandle<Wry>) -> Result<(), String> {
    let Some(url) = policy_url() else {
        debug!("licensing: no policy URL, skipping the check");
        return Ok(());
    };
    let Some(public) = public_key() else {
        return Ok(());
    };

    let client = reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(20))
        .build()
        .map_err(|e| e.to_string())?;

    let blob = client
        .get(&url)
        .send()
        .await
        .and_then(|response| response.error_for_status())
        .map_err(|e| format!("Could not reach the update check: {e}"))?
        .text()
        .await
        .map_err(|e| format!("Could not read the update check: {e}"))?;

    let fetched = Policy::verify(&blob, &public)
        .map_err(|e| format!("The update check answered with something unsigned: {e}"))?;

    for unknown in fetched.unknown_features() {
        debug!("licensing: policy names feature {unknown:?}, which this build does not know");
    }

    let now = now_unix();
    let accepted = {
        let state = app.state::<LicensingState>();
        let mut inner = state.inner.write().unwrap();
        inner.last_checked = Some(now);

        let cached_issued = inner.policy.issued;
        if fetched.issued < cached_issued {
            warn!(
                "licensing: served policy is older than the cached one ({} < {cached_issued}), keeping the cached one",
                fetched.issued
            );
            false
        } else {
            inner.policy = fetched;
            true
        }
    };

    // Written outside the lock: saving touches the store on disk, and holding
    // the write lock across that blocks every feature check in the meantime.
    let mut stored = Stored::load(app);
    if accepted {
        stored.policy = Some(blob);
    }
    stored.last_checked = Some(now);
    stored.save(app);

    enforce(app).await;
    broadcast(app);
    Ok(())
}

/// Puts the wall up when the installed version has fallen below the floor.
async fn enforce(app: &AppHandle<Wry>) {
    let state = app.state::<LicensingState>();
    if let UpdateState::UpdateRequired { minimum } = state.update_state(now_unix()) {
        info!("licensing: this build is below the required {minimum}, blocking");
        let _ = ShowCapWindow::UpdateRequired.show(app).await;
    }
}

pub fn spawn_background_loop(app: AppHandle<Wry>) {
    if policy_url().is_none() {
        return;
    }

    tauri::async_runtime::spawn(async move {
        // The cached policy already decides the first minute; showing the wall
        // before the app has finished starting only produces a window behind
        // the splash.
        tokio::time::sleep(Duration::from_secs(5)).await;
        enforce(&app).await;

        loop {
            if let Err(e) = refresh_policy(&app).await {
                debug!("licensing: policy check failed, keeping the cached one: {e}");
            }
            tokio::time::sleep(REFRESH_INTERVAL).await;
        }
    });
}
