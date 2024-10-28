//! Contains all of the structs related to change requests, responses and more.

use std::{error::Error, fmt::Display};

use lazy_async_promise::BoxedSendError;
use tokio::sync::{
    mpsc,
    oneshot::{self, error::RecvError},
};

use super::{GetKeys, KeyBounds, ValueBounds};

pub(crate) struct Change<Key, Value>
where
    Key: KeyBounds,
    Value: ValueBounds<Key>,
{
    pub reponse_sender: oneshot::Sender<ChangeResult>,
    pub action: ChangeType<Key, Value>,
}

impl<Key, Value> Change<Key, Value>
where
    Key: KeyBounds,
    Value: ValueBounds<Key>,
{
    pub fn from_type(
        action_type: ChangeType<Key, Value>,
    ) -> (Self, oneshot::Receiver<ChangeResult>) {
        let (sender, reciver) = oneshot::channel::<ChangeResult>();

        (
            Self {
                reponse_sender: sender,
                action: action_type,
            },
            reciver,
        )
    }

    //pub(crate) fn all_keys(&self) -> Vec<&Key> {
    //    match &self.action {
    //        ChangeType::Insert(val) => vec![val.key()],
    //        ChangeType::InsertMany(vals) => vals.keys(),
    //        ChangeType::Update(val) => vec![val.key()],
    //        ChangeType::UpdateMany(vals) => vals.keys(),
    //        ChangeType::Delete(key) => vec![key],
    //        ChangeType::DeleteMany(keys) => keys.keys(),
    //    }
    //}
}

pub enum ChangeType<Key, Value>
where
    Key: KeyBounds,
    Value: ValueBounds<Key>,
{
    Insert(Value),
    InsertMany(Vec<Value>),
    Update(Value),
    UpdateMany(Vec<Value>),
    Delete(Key),
    DeleteMany(Vec<Key>),
}

impl<Key, Value> ChangeType<Key, Value>
where
    Key: KeyBounds,
    Value: ValueBounds<Key>,
{
    pub fn is_empty(&self) -> bool {
        match self {
            ChangeType::InsertMany(vals) => vals.is_empty(),
            ChangeType::UpdateMany(vals) => vals.is_empty(),
            ChangeType::DeleteMany(vals) => vals.is_empty(),
            _ => false,
        }
    }
}

impl<Key: KeyBounds, Value: ValueBounds<Key>> Display for ChangeType<Key, Value> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Self::Insert(_) => String::from("Insert"),
                Self::InsertMany(vals) => format!("InsertMany({})", vals.len()),
                Self::Update(_) => String::from("Update"),
                Self::UpdateMany(vals) => format!("UpdateMany({})", vals.len()),
                Self::Delete(_) => String::from("Delete"),
                Self::DeleteMany(vals) => format!("DeleteMany({})", vals.len()),
            }
        )
    }
}

pub enum ChangeResponse<Key: KeyBounds, Value: ValueBounds<Key>> {
    Ok(DataChange<Key, Value>),
    Err(ChangeError),
}

impl<Key, Value> ChangeResponse<Key, Value>
where
    Key: KeyBounds,
    Value: ValueBounds<Key>,
{
    pub fn empty_ok(change_type: ChangeType<Key, Value>) -> Self {
        match change_type {
            ChangeType::Insert(_) | ChangeType::InsertMany(_) => {
                Self::Ok(DataChange::empty_insert())
            }
            ChangeType::Update(_) | ChangeType::UpdateMany(_) => {
                Self::Ok(DataChange::empty_update())
            }
            ChangeType::Delete(_) | ChangeType::DeleteMany(_) => {
                Self::Ok(DataChange::empty_delete())
            }
        }
    }
    pub fn from_type_and_result(
        action_type: ChangeType<Key, Value>,
        action_result: ChangeResult,
    ) -> Self {
        match action_result {
            ChangeResult::Success => Self::Ok(action_type.into()),
            ChangeResult::Error(err) => Self::Err(err),
        }
    }
}

impl<Key, Value> From<ChangeResponse<Key, Value>> for (Option<DataChange<Key, Value>>, ChangeResult)
where
    Key: KeyBounds,
    Value: ValueBounds<Key>,
{
    fn from(value: ChangeResponse<Key, Value>) -> Self {
        match value {
            ChangeResponse::Ok(data) => (Some(data), ChangeResult::Success),
            ChangeResponse::Err(err) => (None, ChangeResult::Error(err)),
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
    DatabaseError(String),
    ChannelSendError(String),
    ChannelReciveError(RecvError),
}

impl ChangeError {
    pub fn send_err<T>(send_err: &mpsc::error::SendError<T>) -> Self {
        Self::ChannelSendError(format!("{send_err}"))
    }
}

impl Display for ChangeError {
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(fmt, "{self:?}")
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

#[derive(Clone)]
pub enum DataChange<Key, Value>
where
    Key: KeyBounds,
    Value: ValueBounds<Key>,
{
    // TODO: maybe add a None option, for when no change is made
    // not sure tho if that should rather be a wrapping of this object
    // in a Option
    Insert(Vec<Value>),
    Update(Vec<Value>),
    Delete(Vec<Key>),
}

impl<Key, Value> DataChange<Key, Value>
where
    Key: KeyBounds,
    Value: ValueBounds<Key>,
{
    pub fn empty_insert() -> Self {
        Self::Insert(vec![])
    }
    pub fn empty_update() -> Self {
        Self::Update(vec![])
    }
    pub fn empty_delete() -> Self {
        Self::Delete(vec![])
    }
    pub fn value_keys(&self) -> Vec<&Key> {
        match self {
            Self::Insert(values) => values.keys(),
            Self::Update(values) => values.keys(),
            Self::Delete(keys) => keys.keys(),
        }
    }

    pub fn len(&self) -> usize {
        match self {
            Self::Insert(values) => values.len(),
            Self::Update(values) => values.len(),
            Self::Delete(keys) => keys.len(),
        }
    }

    pub fn is_empty(&self) -> bool {
        match self {
            Self::Insert(values) => values.is_empty(),
            Self::Update(values) => values.is_empty(),
            Self::Delete(keys) => keys.is_empty(),
        }
    }

    pub fn is_insert(&self) -> bool {
        matches!(self, Self::Insert(_))
    }

    pub fn is_update(&self) -> bool {
        matches!(self, Self::Update(_))
    }

    pub fn is_delete(&self) -> bool {
        matches!(self, Self::Delete(_))
    }
}

impl<Key, Value> From<ChangeType<Key, Value>> for DataChange<Key, Value>
where
    Key: KeyBounds,
    Value: ValueBounds<Key>,
{
    fn from(value: ChangeType<Key, Value>) -> Self {
        match value {
            ChangeType::Insert(val) => Self::Insert(vec![val]),
            ChangeType::InsertMany(vals) => Self::Insert(vals),
            ChangeType::Update(val) => Self::Update(vec![val]),
            ChangeType::UpdateMany(vals) => Self::Update(vals),
            ChangeType::Delete(key) => Self::Delete(vec![key]),
            ChangeType::DeleteMany(keys) => Self::Delete(keys),
        }
    }
}
