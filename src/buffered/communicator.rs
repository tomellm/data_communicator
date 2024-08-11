use std::cmp::Ordering;

use itertools::Itertools;
use lazy_async_promise::{BoxedSendError, ImmediateValuePromise};
use tokio::sync::mpsc;
use tracing::{debug, info, trace, warn};
use uuid::Uuid;

use super::{
    change::{Change, ChangeError, ChangeResult, ChangeType},
    data::{Data, DataChange, FreshData},
    query::{DataQuery, QueryError, QueryResult, QueryType},
    KeyBounds, ValueBounds,
};

pub struct Communicator<Key: KeyBounds, Value: ValueBounds<Key>>
where
    Key: KeyBounds,
    Value: ValueBounds<Key>,
{
    uuid: Uuid,
    sender: Sender<Key, Value>,
    reciver: Reciver<Key, Value>,
    data: Data<Key, Value>,
}

impl<Key, Value> Communicator<Key, Value>
where
    Key: KeyBounds,
    Value: ValueBounds<Key>,
{
    #[must_use]
    pub fn new(
        uuid: Uuid,
        change_sender: mpsc::Sender<Change<Key, Value>>,
        query_sender: mpsc::Sender<DataQuery<Key, Value>>,
        change_data_reciver: mpsc::Receiver<DataChange<Key, Value>>,
        fresh_data_reciver: mpsc::Receiver<FreshData<Key, Value>>,
    ) -> Self {
        let sender = Sender::new(change_sender, query_sender);
        let reciver = Reciver::new(change_data_reciver, fresh_data_reciver);
        Self {
            uuid,
            sender,
            reciver,
            data: Data::new(),
        }
    }
    /// Recives any new updates and then updates the internal data accordingly
    pub fn state_update(&mut self) {
        self.reciver
            .recive_new()
            .into_iter()
            .for_each(|action| match action {
                RecievedAction::Change(update) => update.update_data(&mut self.data),
                RecievedAction::Fresh(data) => data.add_fresh_data(&mut self.data),
            });
    }
    pub fn query(&self, query_type: QueryType<Key, Value>) -> ImmediateValuePromise<QueryResult> {
        trace!("Recived query command.");
        self.sender.send_query(self.uuid, query_type)
    }
    pub fn query_action(&self, query_type: QueryType<Key, Value>) -> impl FnOnce() -> impl std::future::Future<Output = Result<QueryResult, BoxedSendError>> {
        self.sender.send_query_action(self.uuid, query_type)
    }
    /// Sends out an action to update a single element
    pub fn update(&self, val: Value) -> ImmediateValuePromise<ChangeResult> {
        trace!("Recived update command.");
        self.sender.send_change(ChangeType::Update(val))
    }
    pub fn update_many(&self, vals: Vec<Value>) -> ImmediateValuePromise<ChangeResult> {
        trace!("Recived update command.");
        self.sender.send_change(ChangeType::UpdateMany(vals))
    }
    /// Sends out an action to delete a single element
    pub fn delete(&self, key: Key) -> ImmediateValuePromise<ChangeResult> {
        trace!("Recived delete command.");
        self.sender.send_change(ChangeType::Delete(key))
    }
    pub fn delete_many(&self, keys: Vec<Key>) -> ImmediateValuePromise<ChangeResult> {
        trace!("Recived delete many command.");
        self.sender.send_change(ChangeType::DeleteMany(keys))
    }
    pub fn is_empty(&self) -> bool {
        self.data.data.is_empty()
    }
    pub fn len(&self) -> usize {
        self.data.data.len()
    }
    pub fn data(&self) -> Vec<&Value> {
        self.data.data.values().collect_vec()
    }
    pub fn data_iter(&self) -> impl Iterator<Item = &Value> {
        self.data.data.values()
    }
    pub fn data_cloned(&self) -> Vec<Value> {
        self.data.data.values().cloned().collect_vec()
    }
    pub fn data_sorted(&self) -> Vec<&Value> {
        self.data.sorted.apply_slice(self.data())
    }
    pub fn data_sorted_iter(&self) -> impl Iterator<Item = &Value> {
        self.data.sorted.apply_slice(self.data()).into_iter()
    }
    pub fn sort<F: FnMut(&Value, &Value) -> Ordering + 'static>(&mut self, sorting_fn: F) {
        self.data.new_sorting_fn(sorting_fn);
    }
    pub fn keys(&self) -> Vec<&Key> {
        self.data.data.keys().collect_vec()
    }
    pub fn keys_cloned(&self) -> Vec<Key> {
        self.data.data.keys().cloned().collect_vec()
    }
    pub fn keys_iter(&self) -> impl Iterator<Item = &Key> {
        self.data.data.keys()
    }
    pub fn touples(&self) -> Vec<(&Key, &Value)> {
        self.data.data.iter().collect_vec()
    }
}

pub struct Sender<Key, Value>
where
    Key: KeyBounds,
    Value: ValueBounds<Key>,
{
    change_sender: mpsc::Sender<Change<Key, Value>>,
    query_sender: mpsc::Sender<DataQuery<Key, Value>>,
}

impl<Key, Value> Sender<Key, Value>
where
    Key: KeyBounds,
    Value: ValueBounds<Key>,
{
    #[must_use]
    pub fn new(
        change_sender: mpsc::Sender<Change<Key, Value>>,
        query_sender: mpsc::Sender<DataQuery<Key, Value>>,
    ) -> Self {
        Self {
            change_sender,
            query_sender,
        }
    }

    /// Returns a `ImmediateValuePromise` that will resolve to the result of the
    /// action but not to the actual data. The Data will be automatically updated
    // if the result is a success
    pub fn send_change(
        &self,
        action_type: ChangeType<Key, Value>,
    ) -> ImmediateValuePromise<ChangeResult> {
        let new_sender = self.change_sender.clone();
        ImmediateValuePromise::new(async move {
            let action_type_str = format!("{action_type}");
            let (action, reciver) = Change::from_type(action_type);
            let response = match new_sender.send(action).await {
                Ok(()) => {
                    debug!("Change [{action_type_str}] was sent now awaiting response.");
                    reciver.await.into()
                },
                Err(err) => {
                    trace!("Change [{action_type_str}] returned an error [{err}]");
                    ChangeResult::Error(ChangeError::send_err(&err))
                },
            };
            info!("Result for change type [{action_type_str}] was returned, is [{response:?}]");
            Ok(response)
        })
    }

    pub fn send_query(
        &self,
        origin_uuid: Uuid,
        query_type: QueryType<Key, Value>,
    ) -> ImmediateValuePromise<QueryResult> {
        let new_sender = self.query_sender.clone();
        ImmediateValuePromise::new(async move {
            let query_type_str = format!("{query_type}");
            let (query, reciver) = DataQuery::from_type(origin_uuid, query_type);
            let response = match new_sender.send(query).await {
                Ok(()) => {
                    debug!("Query [{query_type_str}] was sent now awaiting response.");
                    reciver.await.into()
                },
                Err(err) => {
                    trace!("Query [{query_type_str}] returned an error [{err}]");
                    QueryResult::Error(QueryError::send(&err))
                },
            };
            info!("Result for query type [{query_type_str}] was returned, is [{response:?}]");
            Ok(response)
        })
    }
    
    pub fn send_query_action(
        &self,
        origin_uuid: Uuid,
        query_type: QueryType<Key, Value>,
    ) -> impl FnOnce() -> impl std::future::Future<Output = Result<QueryResult, BoxedSendError>> {
        let new_sender = self.query_sender.clone();
        move || async move {
            let query_type_str = format!("{query_type}");
            let (query, reciver) = DataQuery::from_type(origin_uuid, query_type);
            let response = match new_sender.send(query).await {
                Ok(()) => {
                    debug!("Query [{query_type_str}] was sent now awaiting response.");
                    reciver.await.into()
                },
                Err(err) => {
                    trace!("Query [{query_type_str}] returned an error [{err}]");
                    QueryResult::Error(QueryError::send(&err))
                },
            };
            info!("Result for query type [{query_type_str}] was returned, is [{response:?}]");
            Ok(response)
        }
    }
}

pub struct Reciver<Key, Value>
where
    Key: KeyBounds,
    Value: ValueBounds<Key>,
{
    change_reciver: mpsc::Receiver<DataChange<Key, Value>>,
    fresh_data_reciver: mpsc::Receiver<FreshData<Key, Value>>,
}

impl<Key, Value> Reciver<Key, Value>
where
    Key: KeyBounds,
    Value: ValueBounds<Key>,
{
    #[must_use]
    pub fn new(
        change_reciver: mpsc::Receiver<DataChange<Key, Value>>,
        fresh_data_reciver: mpsc::Receiver<FreshData<Key, Value>>,
    ) -> Self {
        Self {
            change_reciver,
            fresh_data_reciver,
        }
    }
    /// Tries to recive all new Updates
    #[must_use]
    fn recive_new(&mut self) -> Vec<RecievedAction<Key, Value>> {
        let mut new_updates: Vec<RecievedAction<Key, Value>> = vec![];
        while let Ok(val) = self.change_reciver.try_recv() {
            new_updates.push(val.into());
        }
        while let Ok(val) = self.fresh_data_reciver.try_recv() {
            new_updates.push(val.into());
        }
        new_updates
    }
}

enum RecievedAction<Key, Value>
where
    Key: KeyBounds,
    Value: ValueBounds<Key>,
{
    Change(DataChange<Key, Value>),
    Fresh(FreshData<Key, Value>),
}

impl<Key, Value> From<DataChange<Key, Value>> for RecievedAction<Key, Value>
where
    Key: KeyBounds,
    Value: ValueBounds<Key>,
{
    fn from(value: DataChange<Key, Value>) -> Self {
        Self::Change(value)
    }
}

impl<Key, Value> From<FreshData<Key, Value>> for RecievedAction<Key, Value>
where
    Key: KeyBounds,
    Value: ValueBounds<Key>,
{
    fn from(value: FreshData<Key, Value>) -> Self {
        Self::Fresh(value)
    }
}
