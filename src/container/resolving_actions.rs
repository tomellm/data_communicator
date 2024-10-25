use std::fmt::Display;

use lazy_async_promise::{DirectCacheAccess, ImmediateValuePromise};
use tokio::sync::oneshot;
use tracing::{debug, warn};
use uuid::Uuid;

use crate::{
    change::{Change, ChangeResponse, ChangeResult, DataChange},
    query::{DataQuery, FreshData, QueryResponse, QueryResult},
    utils::PromiseUtilities,
    KeyBounds, ValueBounds,
};

pub enum ResolvingAction<Key, Value>
where
    Key: KeyBounds,
    Value: ValueBounds<Key>,
{
    Change(
        ImmediateValuePromise<ChangeResponse<Key, Value>>,
        oneshot::Sender<ChangeResult>,
    ),
    Query(
        ImmediateValuePromise<QueryResponse<Key, Value>>,
        Uuid,
        oneshot::Sender<QueryResult>,
    ),
}

impl<Key, Value> ResolvingAction<Key, Value>
where
    Key: KeyBounds,
    Value: ValueBounds<Key>,
{
    pub fn poll_and_finished(&mut self) -> bool {
        match self {
            Self::Change(promise, _) => promise.poll_and_check_finished(),
            Self::Query(promise, _, _) => promise.poll_and_check_finished(),
        }
    }

    pub fn resolve(self, cont_uuid: &Uuid) -> Option<ResolvedAction<Key, Value>> {
        match self {
            ResolvingAction::Change(mut promise, sender) => {
                promise.take_value().map(|change_response| {
                    let (data_change, change_result) = change_response.into();
                    let _ = sender.send(change_result).map_err(|value| {
                        warn!(msg = format!("Change result could not be sent because reciver was dropped. Result was: [{value:?}]"), cont = cont_uuid.to_string())
                    });
                    debug!(msg = format!("Sent reponse of change result to communicator"), cont = cont_uuid.to_string());
                    data_change.map(|data| ResolvedAction::Change(data))
                })?
            }
            ResolvingAction::Query(mut promise, uuid, sender) => {
                promise.take_value().map(|query_response| {
                    let (fresh_data, result) = query_response.into();
                    let _ = sender.send(result).map_err(|value| {
                        warn!(msg = format!("Qeuery result could not be sent because reciver was dropped. Result was: [{value:?}]"), cont = cont_uuid.to_string())
                    });
                    debug!(msg = format!("Sent response of query result to communicator [{uuid}]"), cont = cont_uuid.to_string());
                    fresh_data.map(|data| ResolvedAction::Query(data, uuid))
                })?
            }
        }
    }

    pub fn action_type(&self) -> &str {
        match self {
            Self::Change(_, _) => "change",
            Self::Query(_, _, _) => "query",
        }
    }
}

pub enum ResolvedAction<Key, Value>
where
    Key: KeyBounds,
    Value: ValueBounds<Key>,
{
    Change(DataChange<Key, Value>),
    Query(FreshData<Key, Value>, Uuid),
}

pub enum Action<Key, Value>
where
    Key: KeyBounds,
    Value: ValueBounds<Key>,
{
    Change(Change<Key, Value>),
    Query(DataQuery<Key, Value>),
}

impl<Key, Value> Display for Action<Key, Value>
where
    Key: KeyBounds,
    Value: ValueBounds<Key>,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Self::Change(_) => "Change(..)",
                Self::Query(_) => "Query(..)",
            }
        )
    }
}

impl<Key, Value> From<Change<Key, Value>> for Action<Key, Value>
where
    Key: KeyBounds,
    Value: ValueBounds<Key>,
{
    fn from(value: Change<Key, Value>) -> Self {
        Self::Change(value)
    }
}

impl<Key, Value> From<DataQuery<Key, Value>> for Action<Key, Value>
where
    Key: KeyBounds,
    Value: ValueBounds<Key>,
{
    fn from(value: DataQuery<Key, Value>) -> Self {
        Self::Query(value)
    }
}
