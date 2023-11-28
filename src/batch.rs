use std::{boxed::Box, fmt};

use serde::{
    de::DeserializeOwned,
    Serialize,
};
use serde_json::{Value, value::RawValue};
use thiserror::Error;

use crate::jsonrpc::{JsonRpcError, Request, Response};

/// Error thrown when handling batches of JSON-RPC request and responses.
#[derive(Error, Debug)]
pub enum BatchError {
    /// Thrown if deserialization failed
    #[error(transparent)]
    JsonError(#[from] serde_json::Error),

    #[error(transparent)]
    /// Thrown if a response could not be parsed
    JsonRpcError(#[from] JsonRpcError),

    /// Thrown if the batch is empty.
    EmptyBatch,
}

impl std::fmt::Display for BatchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyBatch => write!(f, "The batch is empty."),
            other => other.fmt(f),
        }
    }
}

/// A batch of JSON-RPC requests.
#[derive(Clone, Debug, Default)]
pub struct BatchRequest {
    requests: Vec<Value>,
}

impl BatchRequest {
    pub fn new() -> Self {
        Self { requests: Vec::new() }
    }


    pub fn with_capacity(capacity: usize) -> Self {
        Self { requests: Vec::with_capacity(capacity) }
    }

    /// Returns the number of requests in the batch.
    pub fn len(&self) -> usize {
        self.requests.len()
    }

    /// Returns whether the batch is empty or not.
    pub fn is_empty(&self) -> bool {
        self.requests.is_empty()
    }


    pub fn add_request<T>(&mut self, method: &str, params: T) -> Result<(), BatchError>
        where
            T: Serialize,
    {
        self.requests.push(serde_json::to_value(&Request::new(0, method, params))?);

        Ok(())
    }

    /// Sets the ids of the requests.
    ///
    /// # Arguments
    ///
    /// `first` - id for the first request in the batch.
    ///
    /// # Panics
    ///
    /// If one of the requests is malformed.
    pub(crate) fn set_ids(&mut self, mut first: u64) -> Result<(), BatchError> {
        let requests = self.requests_mut()?;
        for request in requests {
            *request
                .get_mut("id")
                .expect("Malformed JSON-RPC request: {request}, id is missing.") = first.into();
            first += 1;
        }

        Ok(())
    }

    /// Returns a mutable reference to the underlying JSON-RPC requests.
    ///
    /// # Errors
    ///
    /// Returns `BatchError::EmptyBatch` if the batch is empty.
    pub(crate) fn requests_mut(&mut self) -> Result<&mut [Value], BatchError> {
        (!self.is_empty()).then(move || &mut self.requests[..]).ok_or(BatchError::EmptyBatch)
    }

    /// Returns an immutable reference to the underlying JSON-RPC requests.
    ///
    /// # Errors
    ///
    /// Returns `BatchError::EmptyBatch` if the batch is empty.
    pub(crate) fn requests(&self) -> Result<&[Value], BatchError> {
        (!self.is_empty()).then(|| &self.requests[..]).ok_or(BatchError::EmptyBatch)
    }
}

/// A batch of JSON-RPC responses.
#[derive(Clone, Debug)]
pub struct BatchResponse {
    responses: Vec<(u64, Result<Box<RawValue>, JsonRpcError>)>,
}

impl BatchResponse {
    /// Creates a new batch of JSON-RPC responses.
    ///
    /// # Arguments
    ///
    /// `responses` - vector of JSON-RPC responses.
    pub(crate) fn new(responses: Vec<Response>) -> Self {
        let mut responses = responses
            .into_iter()
            .map(|response| match response {
                Response::Success { id, result } => (id, Ok(result.to_owned())),
                Response::Error { id, error } => (id, Err(error)),
                _ => unreachable!(),
            })
            .collect::<Vec<(u64, Result<Box<RawValue>, JsonRpcError>)>>();
        // Sort the responses by descending id, as the order the requests were issued and the order
        // the responses were given may differ. Order is reversed because we pop elements when
        // retrieving the responses.
        responses.sort_by_key(|(id, _)| std::cmp::Reverse(*id));

        Self { responses }
    }

    /// Returns the id of the batch, that is the id of the first response.
    pub(crate) fn id(&self) -> Result<u64, BatchError> {
        // The id of the first request in the batch, be it successful or not, corresponds to the
        // id of the channel to send the response into.
        self.responses.last().map(|(id, _)| *id).ok_or(BatchError::EmptyBatch)
    }

    /// Returns the next response in the batch or `None` if the batch is empty.
    ///
    /// # Errors
    ///
    /// Returns the error corresponding to a given JSON-RPC request if it failed.
    pub fn next_response<T>(&mut self) -> Option<Result<T, BatchError>>
        where
            T: DeserializeOwned,
    {
        // The order is reversed.
        let item = self.responses.pop();
        // Deserializes and returns the response.
        item.map(|(_, body)| {
            body.map_err(Into::into)
                .and_then(|res| serde_json::from_str::<T>(res.get()).map_err(Into::into))
        })
    }

    /// Returns the number of responses contained in the batch.
    pub fn len(&self) -> usize {
        self.responses.len()
    }

    /// Returns whether the batch is empty or not.
    pub fn is_empty(&self) -> bool {
        self.responses.is_empty()
    }
}