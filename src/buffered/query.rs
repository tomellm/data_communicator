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

pub type Predicate<Value> = Box<dyn Fn(&Value) -> bool + Send>;

pub enum QueryType<Key, Value>
where
    Key: KeyBounds,
    Value: ValueBounds<Key>,
{
    All,
    GetById(Key),
    GetByIds(Vec<Key>),
    Predicate(Predicate<Value>),
}

impl<Key, Value> Display for QueryType<Key, Value> 
where
    Key: KeyBounds,
    Value: ValueBounds<Key>,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", match self {
            Self::All => String::from("All"),
            Self::GetById(_) => String::from("GetById"),
            Self::GetByIds(vals) => format!("GetByIds({})", vals.len()),
            Self::Predicate(_) => String::from("Predicate"),
        })
    }
}



impl<Key, Value> QueryType<Key, Value>
where
    Key: KeyBounds,
    Value: ValueBounds<Key>,
{
    pub fn predicate<T: Fn(&Value) -> bool + Send + 'static>(pred: T) -> Self {
        Self::Predicate(Box::new(pred))
    }
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

impl<V, E, Key, Value> From<Result<V, E>> for QueryResponse<Key, Value>
where
    V: Into<FreshData<Key, Value>>,
    E: Into<QueryError>,
    Key: KeyBounds,
    Value: ValueBounds<Key>,
{
    fn from(value: Result<V, E>) -> Self {
        match value {
            Ok(val) => QueryResponse::Ok(val.into()),
            Err(err) => QueryResponse::Err(err.into())
        }
    }
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
