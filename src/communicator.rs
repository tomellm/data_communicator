pub mod data;

use std::cmp::Ordering;

use data::Data;
use futures::future::BoxFuture;
use itertools::Itertools;
use lazy_async_promise::BoxedSendError;
use tokio::sync::mpsc;
use tracing::{debug, info, trace};
use uuid::Uuid;

use crate::{change::DataChange, query::FreshData};

use super::{
    change::{Change, ChangeError, ChangeResult, ChangeType},
    query::{DataQuery, QueryError, QueryResult, QueryType},
    KeyBounds, ValueBounds,
};

/// The struct through which you view and change the data.
///
/// #### View
/// To view the data the communicator has stored first perfome some kind of Query
/// which can be done with the [`query`][Communicator::query] function. As for the
/// available queries check out [`QueryType`].
/// ```
/// let _ = comm.query(QueryType::All).await;
/// ```
/// After quering the data can be accessed through the `data` property. Chech out the [`Data`]
/// struct for more information on accessing the data. 
/// ```
/// for (key, value) in comm.data.map() {
///     // -- your code
/// }
/// ```
/// You can also view the data in a sorted manner. For that first set a sorting
/// function with the [`sort`][Communicator::sort] function and then view the 
/// sorted data by using the [`sorted`][Data::sorted] function on the [`Data`] object.
/// ```
/// // Please note that this only needs to be called once.
/// comm.sort(|a: &Value, b: &Value| a.key().cmp(b.key()));
///
/// for value in comm.data.sorted() {
///     // -- your code
/// }
/// ```
/// #### Change
/// For this simply use one of the `insert`, `update` and `delete` or the related
/// `_many` functions. Otherwise you can also use the `_action` functions to recvive
/// a functions that will perform the change at a later time.
///
/// #### reacting to change
/// Internally the Communicator has a flag that toggles on if the data has changed.
/// This flag can be retrived with the [`has_changed`][Communicator::has_changed]
/// function. To tell the communicator that the data has been seen the [`set_viewed`][Communicator::set_viewed]
/// function can be used. This function also returnes a reference to Self allowing
/// for method chaining.
/// ```
/// if comm.has_changed() {
///     comm.set_viewed()
///         .sort(...)
/// }
/// ```
pub struct Communicator<Key: KeyBounds, Value: ValueBounds<Key>>
where
    Key: KeyBounds,
    Value: ValueBounds<Key>,
{
    uuid: Uuid,
    sender: Sender<Key, Value>,
    reciver: Reciver<Key, Value>,
    pub data: Data<Key, Value>,
    has_changed: bool,
}

impl<Key, Value> Communicator<Key, Value>
where
    Key: KeyBounds,
    Value: ValueBounds<Key>,
{
    #[must_use]
    pub(crate) fn new(
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
            has_changed: true,
        }
    }
    /// Recives any new updates and then updates the internal data accordingly
    pub fn state_update(&mut self) {
        self.reciver.recive_new().into_iter().for_each(|action| {
            match action {
                RecievedAction::Change(update) => self.data.update_data(update),
                RecievedAction::Fresh(data) => self.data.add_fresh_data(data),
            }
            self.has_changed = true;
        });
    }
    pub fn query(
        &self,
        query_type: QueryType<Key, Value>,
    ) -> BoxFuture<'static, Result<QueryResult, BoxedSendError>> {
        trace!("Recived query command.");
        self.sender.send_query(self.uuid, query_type)
    }
    pub fn query_action(
        &self,
        query_type: QueryType<Key, Value>,
    ) -> impl FnOnce() -> BoxFuture<'static, Result<QueryResult, BoxedSendError>> {
        self.sender.send_query_action(self.uuid, query_type)
    }
    pub fn insert(
        &self,
        val: Value,
    ) -> BoxFuture<'static, Result<ChangeResult, BoxedSendError>> {
        trace!("Recived insert command.");
        self.sender
            .send_change(self.uuid, ChangeType::Insert(val))
    }
    pub fn insert_action(
        &self,
    ) -> impl FnMut(Value) -> BoxFuture<'static, Result<ChangeResult, BoxedSendError>> {
        let mut action = self.sender.send_change_action(self.uuid);
        move |value: Value| action(ChangeType::Insert(value))
    }
    pub fn insert_many(
        &self,
        vals: Vec<Value>,
    ) -> BoxFuture<'static, Result<ChangeResult, BoxedSendError>> {
        trace!("Recived insert command.");
        self.sender
            .send_change(self.uuid, ChangeType::InsertMany(vals))
    }
    pub fn insert_many_action(
        &self,
    ) -> impl FnMut(Vec<Value>) -> BoxFuture<'static, Result<ChangeResult, BoxedSendError>> {
        let mut action = self.sender.send_change_action(self.uuid);
        move |values: Vec<Value>| action(ChangeType::InsertMany(values))
    }
    pub fn update(&self, val: Value) -> BoxFuture<'static, Result<ChangeResult, BoxedSendError>> {
        trace!("Recived update command.");
        self.sender.send_change(self.uuid, ChangeType::Update(val))
    }
    pub fn update_action(
        &self,
    ) -> impl FnMut(Value) -> BoxFuture<'static, Result<ChangeResult, BoxedSendError>> {
        let mut action = self.sender.send_change_action(self.uuid);
        move |value: Value| action(ChangeType::Update(value))
    }
    pub fn update_many(
        &self,
        vals: Vec<Value>,
    ) -> BoxFuture<'static, Result<ChangeResult, BoxedSendError>> {
        trace!("Recived update command.");
        self.sender
            .send_change(self.uuid, ChangeType::UpdateMany(vals))
    }
    pub fn update_many_action(
        &self,
    ) -> impl FnMut(Vec<Value>) -> BoxFuture<'static, Result<ChangeResult, BoxedSendError>> {
        let mut action = self.sender.send_change_action(self.uuid);
        move |values: Vec<Value>| action(ChangeType::UpdateMany(values))
    }
    /// Sends out an action to delete a single element
    pub fn delete(&self, key: Key) -> BoxFuture<'static, Result<ChangeResult, BoxedSendError>> {
        trace!("Recived delete command.");
        self.sender.send_change(self.uuid, ChangeType::Delete(key))
    }
    pub fn delete_action(
        &self,
    ) -> impl FnMut(Key) -> BoxFuture<'static, Result<ChangeResult, BoxedSendError>> {
        let mut action = self.sender.send_change_action(self.uuid);
        move |key: Key| action(ChangeType::Delete(key))
    }
    pub fn delete_many(&self, keys: Vec<Key>) -> BoxFuture<'static, Result<ChangeResult, BoxedSendError>> {
        trace!("Recived delete many command.");
        self.sender
            .send_change(self.uuid, ChangeType::DeleteMany(keys))
    }
    pub fn delete_many_action(
        &self,
    ) -> impl FnMut(Vec<Key>) -> BoxFuture<'static, Result<ChangeResult, BoxedSendError>> {
        let mut action = self.sender.send_change_action(self.uuid);
        move |keys: Vec<Key>| action(ChangeType::DeleteMany(keys))
    }
    pub fn is_empty(&self) -> bool {
        self.data.data.is_empty()
    }
    pub fn sort<F: FnMut(&Value, &Value) -> Ordering + Send + 'static>(&mut self, sorting_fn: F) {
        self.data.new_sorting_fn(sorting_fn);
    }
    
    pub fn has_changed(&self) -> bool {
        self.has_changed
    }
    pub fn set_viewed(&mut self) -> &mut Self {
        self.has_changed = false;
        self
    }
    pub fn data(&self) -> Vec<&Value> {
        self.data.data.values().collect_vec()
    }
}

struct Sender<Key, Value>
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
    fn new(
        change_sender: mpsc::Sender<Change<Key, Value>>,
        query_sender: mpsc::Sender<DataQuery<Key, Value>>,
    ) -> Self {
        Self {
            change_sender,
            query_sender,
        }
    }

    fn send_change(
        &self,
        origin_uuid: Uuid,
        action_type: ChangeType<Key, Value>,
    ) -> BoxFuture<'static, Result<ChangeResult, BoxedSendError>> {
        let new_sender = self.change_sender.clone();
        Box::pin(Self::change_future(origin_uuid, new_sender, action_type))
    }

    fn send_change_action(
        &self,
        origin_uuid: Uuid,
    ) -> impl FnMut(ChangeType<Key, Value>) -> BoxFuture<'static, Result<ChangeResult, BoxedSendError>>
    {
        let new_sender = self.change_sender.clone();
        move |action_type: ChangeType<Key, Value>| {
            let cloned_sender = new_sender.clone();
            Box::pin(Self::change_future(origin_uuid, cloned_sender, action_type))
        }
    }

    fn change_future(
        origin_uuid: Uuid,
        new_sender: mpsc::Sender<Change<Key, Value>>,
        action_type: ChangeType<Key, Value>,
    ) -> impl std::future::Future<Output = Result<ChangeResult, BoxedSendError>> {
        async move {
            let action_type_str = format!("{action_type}");
            let (action, reciver) = Change::from_type(action_type);
            let response = match new_sender.send(action).await {
                Ok(()) => {
                    debug!(
                        msg = format!("Change [{action_type_str}] was sent now awaiting response."),
                        comm = origin_uuid.to_string()
                    );
                    reciver.await.into()
                }
                Err(err) => {
                    trace!(
                        msg = format!("Change [{action_type_str}] returned an error [{err}]"),
                        comm = origin_uuid.to_string()
                    );
                    ChangeResult::Error(ChangeError::send_err(&err))
                }
            };
            info!(
                msg = format!(
                    "Result for change type [{action_type_str}] was returned, is [{response:?}]"
                ),
                comm = origin_uuid.to_string()
            );
            Ok(response)
        }
    }

    fn send_query(
        &self,
        origin_uuid: Uuid,
        query_type: QueryType<Key, Value>,
    ) -> BoxFuture<'static, Result<QueryResult, BoxedSendError>> {
        let new_sender = self.query_sender.clone();
        Box::pin(Self::query_future(new_sender, origin_uuid, query_type))
    }
    fn send_query_action(
        &self,
        origin_uuid: Uuid,
        query_type: QueryType<Key, Value>,
    ) -> impl FnOnce() -> BoxFuture<'static, Result<QueryResult, BoxedSendError>> {
        let new_sender = self.query_sender.clone();
        move || Box::pin(Self::query_future(new_sender, origin_uuid, query_type))
    }

    fn query_future(
        new_sender: mpsc::Sender<DataQuery<Key, Value>>,
        origin_uuid: Uuid,
        query_type: QueryType<Key, Value>,
    ) -> impl std::future::Future<Output = Result<QueryResult, BoxedSendError>> {
        async move {
            let query_type_str = format!("{query_type}");
            let (query, reciver) = DataQuery::from_type(origin_uuid, query_type);
            let response = match new_sender.send(query).await {
                Ok(()) => {
                    debug!(
                        msg = format!("Query [{query_type_str}] was sent now awaiting response."),
                        comm = origin_uuid.to_string()
                    );
                    reciver.await.into()
                }
                Err(err) => {
                    trace!(
                        msg = format!("Query [{query_type_str}] returned an error [{err}]"),
                        comm = origin_uuid.to_string()
                    );
                    QueryResult::Error(QueryError::send(&err))
                }
            };
            info!(
                msg = format!(
                    "Result for query type [{query_type_str}] was returned, is [{response:?}]"
                ),
                comm = origin_uuid.to_string()
            );
            Ok(response)
        }
    }
}

struct Reciver<Key, Value>
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
    fn new(
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
