use std::fs;
use std::path::Path;

use aegis_api::modules::wallet::provider::encrypt_entity_secret;
use anyhow::{bail, Context};
use rand::{rngs::OsRng, RngCore};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    aegis_api::env::load_env();

    let args: Vec<String> = std::env::args().skip(1).collect();
    let command = args.first().map(String::as_str).unwrap_or("check");
    let client = Client::new();
    let ctx = CircleSetup::from_env(client)?;

    match command {
        "check" => ctx.check().await,
        "list" => ctx.list_wallet_sets().await,
        "entity-ciphertext" => ctx.entity_ciphertext(&args[1..]).await,
        "create" => ctx.create_wallet_set(&args[1..]).await,
        "help" | "-h" | "--help" => {
            print_help();
            Ok(())
        }
        other => {
            print_help();
            bail!("unknown command: {other}")
        }
    }
}

struct CircleSetup {
    client: Client,
    base_url: String,
    api_key: Option<String>,
    entity_secret: Option<String>,
    wallet_set_id: Option<String>,
}

impl CircleSetup {
    fn from_env(client: Client) -> anyhow::Result<Self> {
        let base_url = std::env::var("CIRCLE_BASE_URL")
            .unwrap_or_else(|_| "https://api.circle.com".into())
            .trim_end_matches('/')
            .to_owned();
        Ok(Self {
            client,
            base_url,
            api_key: nonempty_env("CIRCLE_API_KEY"),
            entity_secret: nonempty_env("CIRCLE_ENTITY_SECRET"),
            wallet_set_id: nonempty_env("CIRCLE_WALLET_SET_ID"),
        })
    }

    async fn check(&self) -> anyhow::Result<()> {
        println!("Circle real-mode setup");
        println!(
            "- CIRCLE_API_KEY: {}",
            present_label(self.api_key.as_deref())
        );
        println!(
            "- CIRCLE_ENTITY_SECRET: {}",
            present_label(self.entity_secret.as_deref())
        );
        println!(
            "- CIRCLE_WALLET_SET_ID: {}",
            self.wallet_set_id.as_deref().unwrap_or("missing")
        );

        if let Some(entity_secret) = self.entity_secret.as_deref() {
            validate_entity_secret(entity_secret)?;
            println!("- entity secret format: ok");
        }

        if let Some(wallet_set_id) = self.wallet_set_id.as_deref() {
            let wallet_set = self.get_wallet_set(wallet_set_id).await?;
            println!(
                "- wallet set reachable: {} ({})",
                wallet_set.id, wallet_set.custody_type
            );
        }

        Ok(())
    }

    async fn list_wallet_sets(&self) -> anyhow::Result<()> {
        let envelope: WalletSetsEnvelope = self
            .client
            .get(self.endpoint("/w3s/walletSets"))
            .header("Authorization", self.auth_header()?)
            .header("X-Request-Id", Uuid::new_v4().to_string())
            .send()
            .await
            .context("circle wallet-set list request failed")?
            .error_for_status_with_body()
            .await?
            .json()
            .await
            .context("circle wallet-set list response was not valid JSON")?;

        if envelope.data.wallet_sets.is_empty() {
            println!("No Circle wallet sets found.");
            return Ok(());
        }

        for wallet_set in envelope.data.wallet_sets {
            println!(
                "{}\t{}\tcreated {}",
                wallet_set.id,
                wallet_set.custody_type,
                wallet_set.create_date.unwrap_or_else(|| "unknown".into())
            );
        }
        Ok(())
    }

    async fn entity_ciphertext(&self, args: &[String]) -> anyhow::Result<()> {
        let mut generate = false;
        let mut write_env_local = false;
        let mut force = false;

        for arg in args {
            match arg.as_str() {
                "--generate" => generate = true,
                "--write-env-local" => write_env_local = true,
                "--force" => force = true,
                other => bail!("unknown entity-ciphertext option: {other}"),
            }
        }

        if generate && !write_env_local {
            bail!("--generate must be paired with --write-env-local so the raw entity secret is not lost");
        }
        if self.entity_secret.is_some() && generate && !force {
            bail!("CIRCLE_ENTITY_SECRET is already set; use --force to replace it");
        }

        let entity_secret = if generate {
            let generated = generate_entity_secret_hex();
            write_env_key("CIRCLE_ENTITY_SECRET", &generated)?;
            println!("Generated CIRCLE_ENTITY_SECRET and saved it to .env.local.");
            generated
        } else {
            self.entity_secret
                .as_deref()
                .context("CIRCLE_ENTITY_SECRET is required; use --generate --write-env-local to create one")?
                .to_owned()
        };

        validate_entity_secret(&entity_secret)?;
        let public_key = self.fetch_entity_public_key().await?;
        let ciphertext = encrypt_entity_secret(&entity_secret, &public_key)
            .map_err(|e| anyhow::anyhow!(e.to_string()))?;

        println!("Paste this value into Circle's Entity Secret Ciphertext field:");
        println!("{ciphertext}");
        println!("After Circle accepts it, download and store the recovery file securely.");
        Ok(())
    }

    async fn create_wallet_set(&self, args: &[String]) -> anyhow::Result<()> {
        let mut name = "Aegis".to_owned();
        let mut write_env_local = false;
        let mut force = false;
        let mut iter = args.iter();

        while let Some(arg) = iter.next() {
            match arg.as_str() {
                "--name" => {
                    name = iter
                        .next()
                        .context("--name requires a wallet-set name")?
                        .to_owned();
                }
                "--write-env-local" => write_env_local = true,
                "--force" => force = true,
                other => bail!("unknown create option: {other}"),
            }
        }

        if self.wallet_set_id.is_some() && !force {
            bail!("CIRCLE_WALLET_SET_ID is already set; use --force to create another wallet set");
        }

        let public_key = self.fetch_entity_public_key().await?;
        let entity_secret = self
            .entity_secret
            .as_deref()
            .context("CIRCLE_ENTITY_SECRET is required to create a wallet set")?;
        let body = CreateWalletSetReq {
            idempotency_key: Uuid::new_v4().to_string(),
            name,
            entity_secret_ciphertext: encrypt_entity_secret(entity_secret, &public_key)
                .map_err(|e| anyhow::anyhow!(e.to_string()))?,
        };

        let envelope: WalletSetEnvelope = self
            .client
            .post(self.endpoint("/w3s/developer/walletSets"))
            .header("Authorization", self.auth_header()?)
            .header("X-Request-Id", Uuid::new_v4().to_string())
            .json(&body)
            .send()
            .await
            .context("circle wallet-set create request failed")?
            .error_for_status_with_body()
            .await?
            .json()
            .await
            .context("circle wallet-set create response was not valid JSON")?;

        println!(
            "Created Circle wallet set: {} ({})",
            envelope.data.wallet_set.id, envelope.data.wallet_set.custody_type
        );
        if write_env_local {
            write_env_key("CIRCLE_WALLET_SET_ID", &envelope.data.wallet_set.id)?;
            println!("Updated .env.local with CIRCLE_WALLET_SET_ID.");
        }

        Ok(())
    }

    async fn get_wallet_set(&self, wallet_set_id: &str) -> anyhow::Result<WalletSet> {
        let envelope: WalletSetEnvelope = self
            .client
            .get(self.endpoint(&format!("/w3s/walletSets/{wallet_set_id}")))
            .header("Authorization", self.auth_header()?)
            .header("X-Request-Id", Uuid::new_v4().to_string())
            .send()
            .await
            .context("circle wallet-set get request failed")?
            .error_for_status_with_body()
            .await?
            .json()
            .await
            .context("circle wallet-set get response was not valid JSON")?;
        Ok(envelope.data.wallet_set)
    }

    async fn fetch_entity_public_key(&self) -> anyhow::Result<String> {
        #[derive(Deserialize)]
        struct Envelope {
            data: Data,
        }
        #[derive(Deserialize)]
        struct Data {
            #[serde(rename = "publicKey")]
            public_key: String,
        }

        let envelope: Envelope = self
            .client
            .get(self.endpoint("/w3s/config/entity/publicKey"))
            .header("Authorization", self.auth_header()?)
            .header("X-Request-Id", Uuid::new_v4().to_string())
            .send()
            .await
            .context("circle entity public key request failed")?
            .error_for_status_with_body()
            .await?
            .json()
            .await
            .context("circle entity public key response was not valid JSON")?;
        Ok(envelope.data.public_key)
    }

    fn endpoint(&self, path: &str) -> String {
        format!("{}/v1{}", self.base_url, path)
    }

    fn auth_header(&self) -> anyhow::Result<String> {
        Ok(format!(
            "Bearer {}",
            self.api_key
                .as_deref()
                .context("CIRCLE_API_KEY is required")?
        ))
    }
}

trait ResponseExt {
    async fn error_for_status_with_body(self) -> anyhow::Result<reqwest::Response>;
}

impl ResponseExt for reqwest::Response {
    async fn error_for_status_with_body(self) -> anyhow::Result<reqwest::Response> {
        if self.status().is_success() {
            return Ok(self);
        }
        let status = self.status();
        let body = self.text().await.unwrap_or_default();
        bail!(
            "circle request failed with {status}: {}",
            trim_response(&body)
        );
    }
}

#[derive(Serialize)]
struct CreateWalletSetReq {
    #[serde(rename = "entitySecretCiphertext")]
    entity_secret_ciphertext: String,
    #[serde(rename = "idempotencyKey")]
    idempotency_key: String,
    name: String,
}

#[derive(Deserialize)]
struct WalletSetEnvelope {
    data: WalletSetData,
}

#[derive(Deserialize)]
struct WalletSetData {
    #[serde(rename = "walletSet")]
    wallet_set: WalletSet,
}

#[derive(Deserialize)]
struct WalletSetsEnvelope {
    data: WalletSetsData,
}

#[derive(Deserialize)]
struct WalletSetsData {
    #[serde(rename = "walletSets")]
    wallet_sets: Vec<WalletSet>,
}

#[derive(Deserialize)]
struct WalletSet {
    id: String,
    #[serde(rename = "custodyType")]
    custody_type: String,
    #[serde(rename = "createDate")]
    create_date: Option<String>,
}

fn nonempty_env(key: &str) -> Option<String> {
    std::env::var(key)
        .ok()
        .map(|v| v.trim().to_owned())
        .filter(|v| !v.is_empty())
}

fn present_label(value: Option<&str>) -> &'static str {
    if value.is_some() {
        "present"
    } else {
        "missing"
    }
}

fn validate_entity_secret(entity_secret: &str) -> anyhow::Result<()> {
    let entity_secret = entity_secret.trim().trim_start_matches("0x");
    let decoded = hex::decode(entity_secret).context("CIRCLE_ENTITY_SECRET must be hex")?;
    if decoded.len() != 32 {
        bail!("CIRCLE_ENTITY_SECRET must decode to 32 bytes");
    }
    Ok(())
}

fn generate_entity_secret_hex() -> String {
    let mut bytes = [0u8; 32];
    OsRng.fill_bytes(&mut bytes);
    hex::encode(bytes)
}

fn write_env_key(key: &str, value: &str) -> anyhow::Result<()> {
    let root = aegis_api::env::workspace_root();
    let path = root.join(".env.local");
    upsert_env_key(&path, key, value)
}

fn upsert_env_key(path: &Path, key: &str, value: &str) -> anyhow::Result<()> {
    let original = fs::read_to_string(path).unwrap_or_default();
    let mut found = false;
    let mut lines = Vec::new();

    for line in original.lines() {
        if line.trim_start().starts_with(&format!("{key}=")) {
            lines.push(format!("{key}={value}"));
            found = true;
        } else {
            lines.push(line.to_owned());
        }
    }
    if !found {
        if !original.is_empty() && !original.ends_with('\n') {
            lines.push(String::new());
        }
        lines.push(format!("{key}={value}"));
    }

    let mut updated = lines.join("\n");
    updated.push('\n');
    fs::write(path, updated).with_context(|| format!("failed to write {}", path.display()))?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o600))
            .with_context(|| format!("failed to chmod 600 {}", path.display()))?;
    }

    Ok(())
}

fn trim_response(body: &str) -> String {
    let trimmed = body.trim();
    if trimmed.len() > 800 {
        format!("{}...", &trimmed[..800])
    } else {
        trimmed.to_owned()
    }
}

fn print_help() {
    println!(
        "Usage:
  cargo run --bin circle_wallet_setup -- check
  cargo run --bin circle_wallet_setup -- list
  cargo run --bin circle_wallet_setup -- entity-ciphertext --generate --write-env-local
  cargo run --bin circle_wallet_setup -- create [--name Aegis] [--write-env-local] [--force]"
    );
}
