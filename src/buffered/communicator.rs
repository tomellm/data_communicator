use lazy_async_promise::ImmediateValuePromise;
use tokio::sync::mpsc;
use tracing::{debug, info, trace};
use uuid::Uuid;

use super::{
    change::{Change, ChangeError, ChangeResult, ChangeType},
    data::{Data, DataChange, FreshData},
    query::{DataQuery, QueryError, QueryResult, QueryType},
    KeyBounds, ValueBounds,
};

pub struct Communicator<Key: KeyBounds, Value: ValueBounds<Key>> {
    pub uuid: Uuid,
    pub sender: Sender<Key, Value>,
    pub reciver: Reciver<Key, Value>,
    pub data: Data<Key, Value>,
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
                RecievedAction::Fresh(data) => self.data.update_with_fresh(data),
            });
    }
    pub fn query(&self, query_type: QueryType<Key, Value>) -> ImmediateValuePromise<QueryResult> {
        info!("Recived query command.");
        self.sender.send_query(self.uuid, query_type)
    }
    /// Sends out an action to update a single element
    pub fn update(&self, val: Value) -> ImmediateValuePromise<ChangeResult> {
        info!("Recived update command.");
        self.sender.send_change(ChangeType::Update(val))
    }
    /// Sends out an action to delete a single element
    pub fn delete(&self, key: Key) -> ImmediateValuePromise<ChangeResult> {
        info!("Recived delete command.");
        self.sender.send_change(ChangeType::Delete(key))
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
        trace!("At the start of the change communicator send change function.");
        let new_sender = self.change_sender.clone();
        ImmediateValuePromise::new(async move {
            trace!("Creating the change promise.");
            let (action, reciver) = Change::from_type(action_type);
            let response = match new_sender.send(action).await {
                Ok(()) => {
                    debug!("Sent change, now awaiting response.");
                    reciver.await.into()
                },
                Err(err) => ChangeResult::Error(ChangeError::send_err(&err)),
            };
            info!("Change result was returned, is {response:?}");
            Ok(response)
        })
    }

    pub fn send_query(
        &self,
        origin_uuid: Uuid,
        query_type: QueryType<Key, Value>,
    ) -> ImmediateValuePromise<QueryResult> {
        info!("in the send query function");
        let new_sender = self.query_sender.clone();
        info!("after the clone");
        ImmediateValuePromise::new(async move {
            info!("starting the promise");
            let (query, reciver) = DataQuery::from_type(origin_uuid, query_type);
            let response = match new_sender.send(query).await {
                Ok(()) => {
                    debug!("Sent query, now awaiting response.");
                    reciver.await.into()
                },
                Err(err) => {
                    debug!("Recived error from channel send {err}");
                    QueryResult::Error(QueryError::send(&err))
                },
            };
            info!("Query result was returned, is: {response:?}");
            Ok(response)
        })
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
