//! Louis' side of the gate: makes the keypair, mints license keys, signs the
//! policy file that goes on the website.
//!
//! Never shipped with the app. It is behind the `mint` feature so the secret
//! half cannot end up in a release build by accident.
//!
//!   cargo run -p shelf-licensing --features mint --bin shelf-license -- keygen
//!   cargo run -p shelf-licensing --features mint --bin shelf-license -- license --id 1001 --name "Jane Doe"
//!   cargo run -p shelf-licensing --features mint --bin shelf-license -- policy --minimum-version 1.0.0

use clap::{Parser, Subcommand};
use ed25519_dalek::SigningKey;
use rand_core::OsRng;
use shelf_licensing::envelope::{encode_hex, seal};
use shelf_licensing::{Feature, License, Policy, Tier, now_unix};
use std::path::{Path, PathBuf};

#[derive(Parser)]
#[command(about = "Mints Shelf license keys and signs the release policy")]
struct Cli {
    /// Where the secret key lives. Back this up; losing it means every future
    /// key and policy has to be re-issued under a new public key, which in turn
    /// means a new app release.
    #[arg(long, global = true, default_value = "~/.shelf-licensing/secret.key")]
    key_file: String,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Creates the keypair. Prints the public key to paste into the app.
    Keygen {
        /// Overwrites an existing secret key. Every key and policy signed with
        /// the old one stops verifying.
        #[arg(long)]
        force: bool,
    },
    /// Mints one license key for one buyer.
    License {
        /// Order reference, so a support mail can be traced to a payment.
        #[arg(long)]
        id: String,
        /// Shown in the buyer's Settings.
        #[arg(long)]
        name: String,
        #[arg(long, default_value = "pro")]
        tier: String,
        /// Leave unset for a key that never runs out.
        #[arg(long)]
        valid_days: Option<u32>,
    },
    /// Signs the policy file to upload to the website.
    Policy {
        /// Versions below this are refused. Use 0.0.0 to block nobody.
        #[arg(long, default_value = "0.0.0")]
        minimum_version: String,
        /// Days of warning before the floor bites.
        #[arg(long, default_value_t = 0)]
        grace_days: u32,
        /// Feature to put behind a license. Repeatable. "app" makes all of
        /// Shelf paid.
        #[arg(long = "paid")]
        paid: Vec<String>,
        #[arg(long)]
        download_url: Option<String>,
        #[arg(long)]
        buy_url: Option<String>,
        /// One line shown on the update wall.
        #[arg(long)]
        message: Option<String>,
    },
    /// Lists the feature keys this build understands.
    Features,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    let key_file = expand_home(&cli.key_file);

    match cli.command {
        Command::Features => {
            for feature in Feature::ALL {
                println!("{:<20} {}", feature.key(), feature.label());
            }
        }

        Command::Keygen { force } => {
            if key_file.exists() && !force {
                return Err(format!(
                    "{} already exists. Pass --force only if you accept that every key \
                     signed with the old one stops working.",
                    key_file.display()
                )
                .into());
            }
            let signing = SigningKey::generate(&mut OsRng);
            write_secret(&key_file, &signing)?;
            println!("secret key : {}", key_file.display());
            println!(
                "public key : {}",
                encode_hex(signing.verifying_key().as_bytes())
            );
            println!();
            println!("Put the public key in apps/desktop/src-tauri/src/licensing/mod.rs");
            println!("(PUBLIC_KEY) and back up the secret key somewhere off this machine.");
        }

        Command::License {
            id,
            name,
            tier,
            valid_days,
        } => {
            let signing = read_secret(&key_file)?;
            let tier = match tier.as_str() {
                "pro" => Tier::Pro,
                "free" => Tier::Free,
                other => return Err(format!("unknown tier {other:?}, use pro or free").into()),
            };
            let issued = now_unix();
            let license = License {
                id,
                name,
                tier,
                issued,
                expires: valid_days.map(|days| issued + i64::from(days) * 86_400),
            };
            println!("{}", seal(&license, &signing)?);
        }

        Command::Policy {
            minimum_version,
            grace_days,
            paid,
            download_url,
            buy_url,
            message,
        } => {
            let signing = read_secret(&key_file)?;
            semver::Version::parse(&minimum_version).map_err(|e| {
                format!("minimum-version {minimum_version:?} is not a version: {e}")
            })?;
            for key in &paid {
                if Feature::from_key(key).is_none() {
                    return Err(format!(
                        "unknown feature {key:?}. Run `shelf-license features` for the list."
                    )
                    .into());
                }
            }
            let policy = Policy {
                issued: now_unix(),
                minimum_version,
                grace_days,
                paid_features: paid,
                download_url,
                buy_url,
                message,
            };
            println!("{}", seal(&policy, &signing)?);
        }
    }

    Ok(())
}

fn expand_home(path: &str) -> PathBuf {
    match path.strip_prefix("~/") {
        Some(rest) => match std::env::var_os("HOME") {
            Some(home) => PathBuf::from(home).join(rest),
            None => PathBuf::from(path),
        },
        None => PathBuf::from(path),
    }
}

fn write_secret(path: &Path, key: &SigningKey) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, encode_hex(key.as_bytes()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
    }
    Ok(())
}

fn read_secret(path: &Path) -> Result<SigningKey, Box<dyn std::error::Error>> {
    let hex = std::fs::read_to_string(path).map_err(|e| {
        format!(
            "cannot read {}: {e}. Run `shelf-license keygen` first.",
            path.display()
        )
    })?;
    let bytes = (0..hex.trim().len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex.trim()[i..i + 2], 16))
        .collect::<Result<Vec<_>, _>>()?;
    let bytes: [u8; 32] = bytes
        .try_into()
        .map_err(|_| "secret key file is not 32 bytes")?;
    Ok(SigningKey::from_bytes(&bytes))
}
