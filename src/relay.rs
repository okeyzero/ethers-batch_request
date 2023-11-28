use std::str::FromStr;
use std::sync::atomic::{AtomicU64, Ordering};

use reqwest::{Client, Error as ReqwestError};
use serde::{de::DeserializeOwned, Serialize};
use thiserror::Error;
use url::Url;

use crate::{
    jsonrpc::{JsonRpcError, Request, Response},
};
use crate::batch::{BatchError, BatchRequest, BatchResponse};

#[derive(Debug)]
pub struct Relay {
    id: AtomicU64,
    client: Client,
    url: Url,
}

#[derive(Error, Debug)]
/// Error thrown when sending an HTTP request
pub enum RelayError {
    /// Thrown if the request failed
    #[error(transparent)]
    ReqwestError(#[from] ReqwestError),

    #[error(transparent)]
    /// Thrown if the response could not be parsed
    JsonRpcError(#[from] JsonRpcError),

    #[error("Deserialization Error: {err}. Response: {text}")]
    /// Serde JSON Error
    SerdeJson { err: serde_json::Error, text: String },

    /// Thrown if sending an empty batch of JSON-RPC requests.
    #[error(transparent)]
    BatchError(#[from] BatchError),
}


impl Relay {
    /// Initializes a new relay client.
    pub fn new(url: impl Into<Url>) -> Self {
        Self {
            id: AtomicU64::new(0),
            client: Client::new(),
            url: url.into(),
        }
    }


    async fn request<T: Serialize + Send + Sync, R: DeserializeOwned>(
        &self,
        method: &str,
        params: T,
    ) -> Result<R, RelayError> {
        let next_id = self.id.fetch_add(1, Ordering::SeqCst);
        let payload = Request::new(next_id, method, params);

        let res = self.client.post(self.url.as_ref()).json(&payload).send().await?;
        let text = res.text().await?;

        let raw = match serde_json::from_str(&text) {
            Ok(Response::Success { result, .. }) => result.to_owned(),
            Ok(Response::Error { error, .. }) => return Err(error.into()),
            Ok(_) => {
                let err = RelayError::SerdeJson {
                    err: serde::de::Error::custom("unexpected notification over HTTP transport"),
                    text,
                };
                return Err(err);
            }
            Err(err) => return Err(RelayError::SerdeJson { err, text }),
        };

        let res = serde_json::from_str(raw.get())
            .map_err(|err| RelayError::SerdeJson { err, text: raw.to_string() })?;

        Ok(res)
    }

    /// Executes the batch of JSON-RPC requests.
    ///
    /// # Arguments
    ///
    /// `batch` - batch of JSON-RPC requests.
    ///
    /// # Errors
    ///
    /// If `batch` is empty returns errors.
    pub async fn execute_batch(
        &self,
        batch: &mut BatchRequest,
    ) -> Result<BatchResponse, RelayError> {
        let next_id = self.id.fetch_add(batch.len() as u64, Ordering::SeqCst);
        // Ids in the batch will start from next_id.
        batch.set_ids(next_id)?;

        let res = self.client.post(self.url.as_ref()).json(batch.requests()?).send().await?;
        let text = res.text().await?;

        // Get the responses for the batch.
        let responses = serde_json::from_str::<Vec<Response>>(&text)
            .map_err(|err| RelayError::SerdeJson { err, text: text.to_string() })?;

        Ok(BatchResponse::new(responses))
    }
}

impl FromStr for Relay {
    type Err = url::ParseError;

    fn from_str(src: &str) -> Result<Self, Self::Err> {
        let url = Url::parse(src)?;
        Ok(Relay::new(url))
    }
}

impl Clone for Relay {
    fn clone(&self) -> Self {
        Self { id: AtomicU64::new(1), client: self.client.clone(), url: self.url.clone() }
    }
}
