use anyhow::{anyhow, Context, Result};
use solana_sdk::signature::Keypair;
use wallet_adapter_common::storage::KeypairStorage;
use web_sys::Storage;

#[derive(Debug)]
pub enum WasmStorageType {
    Local,
    Session,
}

#[derive(Debug)]
pub struct WasmStorage {
    storage_type: WasmStorageType,
}

impl WasmStorage {
    pub fn local() -> Result<Self> {
        Ok(Self {
            storage_type: WasmStorageType::Local,
        })
    }

    pub fn session() -> Result<Self> {
        Ok(Self {
            storage_type: WasmStorageType::Session,
        })
    }

    fn storage(&self) -> Result<Storage> {
        let window = web_sys::window().context("window not available")?;
        let res = match self.storage_type {
            WasmStorageType::Local => window.local_storage(),
            WasmStorageType::Session => window.session_storage(),
        };
        res.map_err(|err| anyhow!("storage not available: {err:?}"))?
            .context("storage not available")
    }
}

impl KeypairStorage for WasmStorage {
    fn get_keypair(&self) -> Result<Option<Keypair>> {
        let item = self
            .storage()?
            .get_item("keypair")
            .map_err(|err| anyhow!("{err:?}"))?;
        match item {
            Some(item) => Ok(Some(Keypair::from_bytes(&hex::decode(item)?)?)),
            None => Ok(None),
        }
    }

    fn set_keypair(&self, keypair: Keypair) -> Result<()> {
        self.storage()?
            .set_item("keypair", &hex::encode(keypair.to_bytes()))
            .map_err(|err| anyhow!("{err:?}"))?;

        Ok(())
    }
}
