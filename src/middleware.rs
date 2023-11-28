use ethers::{
    providers::{Middleware, MiddlewareError}
};
use thiserror::Error;
use url::Url;

use crate::{
    relay::{Relay, RelayError}
};
use crate::batch::{BatchRequest, BatchResponse};

/// Errors for the Flashbots middleware.
#[derive(Error, Debug)]
pub enum BatchRequestMiddlewareError<M: Middleware> {
    #[error("Some parameters were missing")]
    MissingParameters,
    /// The relay responded with an error.
    #[error(transparent)]
    RelayError(#[from] RelayError),
    /// An error occured in one of the middlewares.
    #[error("{0}")]
    MiddlewareError(M::Error),
}

impl<M: Middleware> MiddlewareError for BatchRequestMiddlewareError<M> {
    type Inner = M::Error;

    fn from_err(src: M::Error) -> BatchRequestMiddlewareError<M> {
        BatchRequestMiddlewareError::MiddlewareError(src)
    }

    fn as_inner(&self) -> Option<&Self::Inner> {
        match self {
            BatchRequestMiddlewareError::MiddlewareError(e) => Some(e),
            _ => None,
        }
    }
}

/// # Example
/// ```
/// use ethers::prelude::*;
/// use std::convert::TryFrom;
/// use crate::BatchRequestMiddleware;
/// use url::Url;
///
/// # async fn foo() -> Result<(), Box<dyn std::error::Error>> {
/// let provider = Provider::<Http>::try_from("http://localhost:8545")
///     .expect("Could not instantiate HTTP provider");
///
/// // Used to sign Flashbots relay requests - this is your searcher identity
/// let signer: LocalWallet = "380eb0f3d505f087e438eca80bc4df9a7faa24f868e69fc0440261a0fc0567dc"
///     .parse()?;
///
/// // Used to sign transactions
/// let wallet: LocalWallet = "380eb0f3d505f087e438eca80bc4df9a7faa24f868e69fc0440261a0fc0567dc"
///     .parse()?;
///
/// // Note: The order is important! You want the signer
/// // middleware to sign your transactions *before* they
/// // are sent to your Flashbots middleware.
/// let mut client = SignerMiddleware::new(
///     BatchRequestMiddlewareError::new(
///         provider,
///         Url::parse("https://relay.flashbots.net")?,
///         signer
///     ),
///     wallet
/// );
///
/// // This transaction will now be send as a Flashbots bundle!
/// let tx = TransactionRequest::pay("vitalik.eth", 100);
/// let pending_tx = client.send_transaction(tx, None).await?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug)]
pub struct BatchRequestMiddleware<M> {
    inner: M,
    relay: Relay,
}

impl<M: Middleware> BatchRequestMiddleware<M> {
    /// Initialize a new BatchRequest middleware.
    pub fn new(inner: M, relay_url: impl Into<Url>) -> Self {
        Self {
            inner,
            relay: Relay::new(relay_url),
        }
    }

    /// Get the relay client used by the middleware.
    pub fn relay(&self) -> &Relay {
        &self.relay
    }

    pub async fn execute_batch(
        &self,
        bundle: &mut BatchRequest,
    ) -> Result<BatchResponse, BatchRequestMiddlewareError<M>>
    {
        let response: BatchResponse = self
            .relay
            .execute_batch(bundle)
            .await
            .map_err(BatchRequestMiddlewareError::RelayError)?;

        Ok(response)
    }
}