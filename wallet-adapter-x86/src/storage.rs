use std::fs::File;

use anyhow::{Context, Result};
use platform_dirs::AppDirs;
use solana_sdk::signature::Keypair;
use wallet_adapter_common::storage::KeypairStorage;

#[derive(Debug)]
pub struct X86Storage {
    config_dir_name: String,
}

impl X86Storage {
    pub fn new(config_dir_name: impl ToString) -> Result<Self> {
        Ok(Self {
            config_dir_name: config_dir_name.to_string(),
        })
    }
}

impl KeypairStorage for X86Storage {
    fn get_keypair(&self) -> Result<Option<Keypair>> {
        let app_dirs =
            AppDirs::new(Some(&self.config_dir_name), true).context("Unable to get app dirs")?;
        let config_file_path = app_dirs.config_dir.join("key.json");

        let file = if config_file_path.exists() {
            File::open(config_file_path)?
        } else {
            return Ok(None);
        };

        let keypair_str: String =
            serde_json::from_reader(file).context("Unable to read keypair from file")?;

        // TODO: this panics if the keypair_str is invalid
        Ok(Some(Keypair::from_base58_string(&keypair_str)))
    }

    fn set_keypair(&self, keypair: Keypair) -> Result<()> {
        let app_dirs =
            AppDirs::new(Some(&self.config_dir_name), true).context("Unable to get app dirs")?;
        let config_file_path = app_dirs.config_dir.join("key.json");
        std::fs::create_dir_all(&app_dirs.config_dir).unwrap();

        let file = if config_file_path.exists() {
            File::open(config_file_path)?
        } else {
            File::create(config_file_path)?
        };

        serde_json::to_writer(file, &keypair.to_base58_string())
            .context("Unable to write keypair to file")?;

        Ok(())
    }
}
