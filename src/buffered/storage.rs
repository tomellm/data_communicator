use futures::future::BoxFuture;
use lazy_async_promise::ImmediateValuePromise;

use super::{
    change::{ChangeResponse, ChangeResult, ChangeType},
    query::{Predicate, QueryResponse, QueryType},
    KeyBounds, ValueBounds,
};

pub trait Storage<Key: KeyBounds, Value: ValueBounds<Key>>
where
    Self: Send + Sync,
    Key: KeyBounds,
    Value: ValueBounds<Key>,
{
    type InitArgs;
    fn init(args: Self::InitArgs) -> impl InitFuture<Self>;
    fn handle_change(
        &mut self,
        action: ChangeType<Key, Value>,
    ) -> ImmediateValuePromise<ChangeResponse<Key, Value>> {
        let action_future = match &action {
            ChangeType::Update(value) => to_boxed(self.update(value)),
            ChangeType::UpdateMany(values) => to_boxed(self.update_many(values)),
            ChangeType::Delete(key) => to_boxed(self.delete(key)),
            ChangeType::DeleteMany(values) => to_boxed(self.delete_many(values)),
        };
        ImmediateValuePromise::new(async move {
            Ok(ChangeResponse::from_type_and_result(
                action,
                action_future.await,
            ))
        })
    }
    fn update(&mut self, value: &Value) -> impl Future<ChangeResult>;
    fn update_many(&mut self, values: &Vec<Value>) -> impl Future<ChangeResult>;
    fn delete(&mut self, key: &Key) -> impl Future<ChangeResult>;
    fn delete_many(&mut self, keys: &Vec<Key>) -> impl Future<ChangeResult>;
    fn handle_query(
        &mut self,
        query: QueryType<Key, Value>,
    ) -> ImmediateValuePromise<QueryResponse<Key, Value>> {
        let query_future = match query {
            QueryType::GetById(id) => to_boxed(self.get_by_id(id)),
            QueryType::GetByIds(ids) => to_boxed(self.get_by_ids(ids)),
            QueryType::Predicate(pred) => to_boxed(self.get_by_predicate(pred)),
        };
        ImmediateValuePromise::new(async move { Ok(query_future.await) })
    }
    fn get_by_id(&mut self, key: Key) -> impl Future<QueryResponse<Key, Value>>;
    // TODO: this function could technically have a default implementation
    // where it just uses the predicate function to do a search
    fn get_by_ids(&mut self, keys: Vec<Key>) -> impl Future<QueryResponse<Key, Value>>;
    fn get_by_predicate(
        &mut self,
        predicate: Predicate<Value>,
    ) -> impl Future<QueryResponse<Key, Value>>;
}

pub trait InitFuture<FutOutput>
where
    Self: std::future::Future<Output = FutOutput> + Send + Sync + 'static,
    FutOutput: Send + Sync + ?Sized,
{}

impl<T, FutOutput> InitFuture<FutOutput> for T 
where
    T: std::future::Future<Output = FutOutput> + Send + Sync + 'static,
    FutOutput: Send + Sync + ?Sized,
{}

pub trait Future<FutureOutput>
where
    Self: std::future::Future<Output = FutureOutput> + Send + Sync + 'static,
    FutureOutput: Clone + Send + Sync,
{
}

impl<T, FutOutput> Future<FutOutput> for T
where
    T: std::future::Future<Output = FutOutput> + Send + Sync + 'static,
    FutOutput: Clone + Send + Sync,
{
}

fn to_boxed<FutOutput>(fut: impl Future<FutOutput>) -> BoxFuture<'static, FutOutput>
where
    FutOutput: Clone + Send + Sync + 'static,
{
    Box::pin(fut) as BoxFuture<'static, FutOutput>
}