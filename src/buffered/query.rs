use std::{error::Error, fmt::Display};

use tokio::sync::{
    mpsc,
    oneshot::{self, error::RecvError},
};
use uuid::Uuid;

use super::{data::FreshData, KeyBounds, ValueBounds};

pub struct DataQuery<Key, Value>
where
    Key: KeyBounds,
    Value: ValueBounds<Key>,
{
    pub origin_uuid: Uuid,
    pub response_sender: oneshot::Sender<QueryResult>,
    pub query_type: QueryType<Key, Value>,
}

impl<Key, Value> DataQuery<Key, Value>
where
    Key: KeyBounds,
    Value: ValueBounds<Key>,
{
    pub fn from_type(
        origin_uuid: Uuid,
        query_type: QueryType<Key, Value>,
    ) -> (Self, oneshot::Receiver<QueryResult>) {
        let (sender, reciver) = oneshot::channel::<QueryResult>();
        (
            Self {
                origin_uuid,
                response_sender: sender,
                query_type,
            },
            reciver,
        )
    }
}

pub type Predicate<Value> = Box<dyn Fn(&Value) -> bool + Send + Sync>;

pub enum QueryType<Key, Value>
where
    Key: KeyBounds,
    Value: ValueBounds<Key>,
{
    GetById(Key),
    GetByIds(Vec<Key>),
    Predicate(Predicate<Value>),
}

#[derive(Clone)]
pub enum QueryResponse<Key, Value>
where
    Key: KeyBounds,
    Value: ValueBounds<Key>,
{
    // TODO: I dont think that the hashmap is needed. A vec should be enought
    // but the compiler doesnt allow me to keep the Key generic If I dont use it.
    // Same problem as this one: https://internals.rust-lang.org/t/type-parameter-not-used-on-enums/13342
    Ok(FreshData<Key, Value>),
    Err(QueryError),
}

impl<Key, Value> From<QueryResponse<Key, Value>> for (Option<FreshData<Key, Value>>, QueryResult)
where
    Key: KeyBounds,
    Value: ValueBounds<Key>,
{
    fn from(value: QueryResponse<Key, Value>) -> Self {
        match value {
            QueryResponse::Ok(fresh_data) => (Some(fresh_data), QueryResult::Success),
            QueryResponse::Err(err) => (None, QueryResult::Error(err)),
        }
    }
}

#[derive(Clone, Debug)]
pub enum QueryResult {
    Success,
    Error(QueryError),
}

#[derive(Debug, Clone)]
pub enum QueryError {
    Default,
    NotPresent,
    ChannelSend(String),
    ChannelTrySend(String),
    ChannelRecive(RecvError),
}

impl QueryError {
    pub fn send<T>(send_err: &mpsc::error::SendError<T>) -> Self {
        Self::ChannelSend(format!("{send_err}"))
    }
    pub fn try_send<T>(send_err: &mpsc::error::TrySendError<T>) -> Self {
        Self::ChannelSend(format!("{send_err}"))
    }

}

impl Display for QueryError {
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(fmt, "{self:?}")
    }
}
impl Error for QueryError {}

impl From<Result<QueryResult, RecvError>> for QueryResult {
    fn from(value: Result<QueryResult, RecvError>) -> Self {
        match value {
            Ok(result) => result,
            Err(err) => QueryResult::Error(QueryError::ChannelRecive(err)),
        }
    }
}
