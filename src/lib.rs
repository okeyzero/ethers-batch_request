mod jsonrpc;
mod relay;
mod middleware;
mod batch;


#[cfg(test)]
mod tests {
    use ethers::prelude::*;
    use ethers::utils::{Anvil, hex};
    use url::Url;

    use crate::batch::{BatchRequest, BatchResponse};
    use crate::middleware::BatchRequestMiddleware;

    use super::*;

    #[tokio::test]
    async fn test_batch() -> Result<(), Box<dyn std::error::Error>> {
        let address1: Address = "0xd5a37dC5C9A396A03dd1136Fc76A1a02B1c88Ffa".parse()?;
        let address2: Address = "0x4d7450da3abc67663c6C3989B4c84F35e5D0B5fC".parse()?;

        let pos = H256::from_low_u64_be(8);

        let mut batch = BatchRequest::with_capacity(2);
        batch.add_request("eth_getStorageAt", (address1, pos, BlockNumber::Latest))?;
        batch.add_request("eth_getStorageAt", (address2, pos, BlockNumber::Latest))?;

        let rpc = "https://api.avax.network/ext/bc/C/rpc";
        let http_client = Provider::<Http>::try_from(rpc)?;
        let client = BatchRequestMiddleware::new(http_client, Url::parse("https://api.avax.network/ext/bc/C/rpc")?);

        // or use relay
        // let relay = relay::Relay::new(Url::parse("https://api.avax.network/ext/bc/C/rpc")?);
        // let mut http_responses = relay.execute_batch(&mut batch).await?;

        let mut http_responses: BatchResponse = client.execute_batch(&mut batch).await?;

        while let Some(Ok(storage)) = http_responses.next_response::<H256>() {
            println!("{storage:?}")
        }

        let rpc_url="https://ethereum-goerli.publicnode.com";


        let relay = relay::Relay::new(Url::parse(rpc_url)?);


        let provider = Provider::<Http>::try_from(rpc_url)?;
        let chain_id = provider.get_chainid().await?;
        let private_key = "380eb0f3d505f087e438eca80bc4df9a7faa24f868e69fc0440261a0fc0567dc";
        let wallet = private_key.parse::<LocalWallet>().unwrap().with_chain_id(chain_id.as_u64());
        let address = wallet.address();
        let mut nonce = provider.get_transaction_count(address, None).await?;
        let mut batch = BatchRequest::new();
        for i in 0..2 {
            nonce = nonce + i;
            let mut tx = TransactionRequest::new()
                .chain_id(chain_id.as_u64())
                .from(address)
                .to(address)
                .value(1000)
                .nonce(nonce)
                .gas(21000)
                .gas_price(10e9 as u64)
                .into();
            provider.fill_transaction(&mut tx, None).await?;
            let signature = wallet.sign_transaction_sync(&tx)?;
            let hash = tx.hash(&signature);
            //println!("hash: {:?}", hash);
            let a = String::from("0x");
            let mut signed_tx =  hex::encode(tx.rlp_signed(&signature));
            signed_tx = a + &signed_tx;
            println!("signed_tx: {:?}", signed_tx);

            batch.add_request("eth_sendRawTransaction", (vec![signed_tx]))?;
        }

        let mut  http_responses = relay.execute_batch(&mut batch).await?;
        println!("http_responses: {:?}", http_responses);

        while let Some(Ok(storage)) = http_responses.next_response::<H256>() {
            println!("{storage:?}")
        }










        Ok(())
    }

}
