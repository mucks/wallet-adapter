//! types that the solana wallet adapter uses
//! `solana-sdk` doesn't have all the types the `web3.js` has so we need to define our own

use anyhow::Result;
use serde::{Deserialize, Serialize};
use solana_sdk::{
    commitment_config::CommitmentLevel,
    hash::Hash,
    pubkey::Pubkey,
    signature::{Keypair, Signature},
    signer::Signer,
};

#[async_trait::async_trait(?Send)]
pub trait Connection {
    async fn get_recent_blockhash(
        &self,
        commitment: Option<CommitmentLevel>,
        min_context_slots: Option<u32>,
    ) -> Result<Hash>;

    async fn send_raw_transaction(
        &self,
        raw_transaction: Vec<u8>,
        options: Option<&SendTransactionOptions>,
    ) -> Result<Signature>;
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SendOptions {
    /** disable transaction verification step */
    pub skip_preflight: Option<bool>,
    /** preflight commitment level */
    pub preflight_commitment: Option<CommitmentLevel>,
    /** Maximum number of times for the RPC node to retry sending the transaction to the leader. */
    pub max_retries: Option<u32>,
    /** The minimum slot that the request can be evaluated at */
    pub min_context_slots: Option<u32>,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SendTransactionOptions {
    #[serde(skip)]
    pub signers: Vec<Box<dyn Signer>>,
    #[serde(flatten)]
    pub send_options: SendOptions,
}

#[cfg(test)]
mod tests {
    use anyhow::Result;
    use solana_sdk::{instruction::Instruction, transaction::Transaction};

    use super::*;

    #[test]
    fn test_instruction() -> Result<()> {
        let idl_bytes =
            include_bytes!("../../examples/all-wallets-base-ui/test_data/anchor_playground.json");
        let idl = anchor_lang_idl::convert::convert_idl(idl_bytes).unwrap();

        let program_id: Pubkey = idl.address.parse().unwrap();

        let data = idl.instructions[0].discriminator.clone();
        let instruction = Instruction::new_with_bytes(program_id, &data, vec![]);

        let blockhash = "2AqazpAqDQJYACzBnJ1PZTp681zUYshU33Jcap3BHySi";

        let payer: Pubkey = "8ZR5P5Xr7uJc6qG4dFseMaoRuNZQiZ4i8ycWqPWxy7Vw".parse()?;

        let mut tx = Transaction::new_unsigned(solana_sdk::message::Message::new(
            &[instruction],
            Some(&payer),
        ));

        tx.message.recent_blockhash = blockhash.parse()?;
        tx.signatures = vec![Signature::default()];

        let expected_tx_hex = "010000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000001000102704f9ddd8c7fb1999e8b487b63d08381eefdd2f2fd740f478227558481636436d995c11a1c4d39d995b2ec1c0ef6910737bb8dbcb9bd5b13563887be368522f91160d9ca64f164ebfefdc52e706945b2145e5462c3bef714c4fdb97343f71b5301010008afaf6d1f0d989bed";

        let tx_hex = hex::encode(&bincode::serialize(&tx)?);

        assert_eq!(tx_hex, expected_tx_hex);

        Ok(())
    }
}
