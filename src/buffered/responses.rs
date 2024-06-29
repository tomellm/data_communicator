use std::{error::Error, fmt::Display};

use lazy_async_promise::BoxedSendError;
use tokio::sync::{mpsc, oneshot::error::RecvError};

use super::{actions::ActionType, data::DataUpdate, KeyBounds, ValueBounds};

pub enum ActionResponse<Key: KeyBounds, Value: ValueBounds<Key>> {
    Ok(DataUpdate<Key, Value>, ActionResult),
    Err(ActionResult),
}

impl<Key: KeyBounds, Value: ValueBounds<Key>> ActionResponse<Key, Value> {
    pub fn from_type_and_result(
        action_type: ActionType<Key, Value>,
        action_result: ActionResult
    ) -> Self {
        match &action_result {
            ActionResult::Success => Self::Ok(action_type.into(), action_result),
            ActionResult::Error(_) => Self::Err(action_result)
        }
    }
}

impl<Key, Value> From<ActionResponse<Key, Value>> for (Option<DataUpdate<Key, Value>>, ActionResult) 
where
    Key: KeyBounds,
    Value: ValueBounds<Key>
{
    fn from(value: ActionResponse<Key, Value>) -> Self {
        match value {
            ActionResponse::Ok(data, res) => (Some(data), res),
            ActionResponse::Err(res) => (None, res)
        }
    }
}

#[derive(Clone, Debug)]
pub enum ActionResult {
    Success,
    Error(ActionError),
}

#[derive(Debug, Clone)]
pub enum ActionError {
    DefaultError,
    ChannelSendError(String),
    ChannelReciveError(RecvError),
}

impl ActionError {
    pub fn send_err<T>(send_err: mpsc::error::SendError<T>) -> Self {
        Self::ChannelSendError(format!("{send_err}"))
    }
}

impl Display for ActionError {
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(fmt, "{:?}", self)
    }
}

impl Error for ActionError {}

impl From<Result<ActionResult, RecvError>> for ActionResult {
    fn from(value: Result<ActionResult, RecvError>) -> Self {
        match value {
            Ok(result) => result,
            Err(err) => ActionResult::Error(ActionError::ChannelReciveError(err)),
        }
    }
}

impl From<ActionResult> for Result<(), BoxedSendError> {
    fn from(value: ActionResult) -> Self {
        match value {
            ActionResult::Success => Ok(()),
            ActionResult::Error(err) => Err(err.into()),
        }
    }
}
