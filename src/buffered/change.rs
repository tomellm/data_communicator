
use std::{error::Error, fmt::Display};

use lazy_async_promise::BoxedSendError;
use tokio::sync::{mpsc, oneshot::{self, error::RecvError}};

use super::{data::DataChange, KeyBounds, ValueBounds};


pub struct Change<Key: KeyBounds, Value: ValueBounds<Key>> {
    pub reponse_sender: oneshot::Sender<ChangeResult>,
    pub action: ChangeType<Key, Value>
}

impl<Key: KeyBounds, Value: ValueBounds<Key>> Change<Key, Value> {
    pub fn from_type(
        action_type: ChangeType<Key, Value>
    ) -> (Self, oneshot::Receiver<ChangeResult>) {
        let (sender, reciver) = oneshot::channel::<ChangeResult>();

        (
            Self { reponse_sender: sender, action: action_type },
            reciver
        )
    }

    pub fn all_keys(&self) -> Vec<&Key> {
        match &self.action {
            ChangeType::Update(val) => vec![val.key()],
            ChangeType::UpdateMany(vals) => vals.iter().map(|v|v.key()).collect(),
            ChangeType::Delete(key) => vec![key],
            ChangeType::DeleteMany(keys) => keys.into_iter().collect()
        }
    }
}

pub enum ChangeType<Key: KeyBounds, Value: ValueBounds<Key>> {
    Update(Value),
    UpdateMany(Vec<Value>),
    Delete(Key),
    DeleteMany(Vec<Key>)
}


pub enum ChangeResponse<Key: KeyBounds, Value: ValueBounds<Key>> {
    Ok(DataChange<Key, Value>),
    Err(ChangeError),
}

impl<Key: KeyBounds, Value: ValueBounds<Key>> ChangeResponse<Key, Value> {
    pub fn from_type_and_result(
        action_type: ChangeType<Key, Value>,
        action_result: ChangeResult
    ) -> Self {
        match action_result {
            ChangeResult::Success => Self::Ok(action_type.into()),
            ChangeResult::Error(err) => Self::Err(err)
        }
    }
}

impl<Key, Value> From<ChangeResponse<Key, Value>> for (Option<DataChange<Key, Value>>, ChangeResult) 
where
    Key: KeyBounds,
    Value: ValueBounds<Key>
{
    fn from(value: ChangeResponse<Key, Value>) -> Self {
        match value {
            ChangeResponse::Ok(data) => (Some(data), ChangeResult::Success),
            ChangeResponse::Err(err) => (None, ChangeResult::Error(err))
        }
    }
}

#[derive(Clone, Debug)]
pub enum ChangeResult {
    Success,
    Error(ChangeError),
}

#[derive(Debug, Clone)]
pub enum ChangeError {
    DefaultError,
    ChannelSendError(String),
    ChannelReciveError(RecvError),
}

impl ChangeError {
    pub fn send_err<T>(send_err: mpsc::error::SendError<T>) -> Self {
        Self::ChannelSendError(format!("{send_err}"))
    }
}

impl Display for ChangeError {
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(fmt, "{:?}", self)
    }
}

impl Error for ChangeError {}

impl From<Result<ChangeResult, RecvError>> for ChangeResult {
    fn from(value: Result<ChangeResult, RecvError>) -> Self {
        match value {
            Ok(result) => result,
            Err(err) => ChangeResult::Error(ChangeError::ChannelReciveError(err)),
        }
    }
}

impl From<ChangeResult> for Result<(), BoxedSendError> {
    fn from(value: ChangeResult) -> Self {
        match value {
            ChangeResult::Success => Ok(()),
            ChangeResult::Error(err) => Err(err.into()),
        }
    }
}
