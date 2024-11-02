use anyhow::Result;
use solana_sdk::signature::Keypair;

pub trait KeypairStorage: std::fmt::Debug + Sync + Send {
    fn get_keypair(&self) -> Result<Option<Keypair>>;
    fn set_keypair(&self, keypair: Keypair) -> Result<()>;
}
