use std::future::Future;

use futures::future::BoxFuture;
use lazy_async_promise::ImmediateValuePromise;

use crate::buffered::actions::ActionType;

use super::{
    responses::{ActionResponse, ActionResult}, KeyBounds, ValueBounds
};

pub trait Storage<Key: KeyBounds, Value: ValueBounds<Key>>
where
    Self: Send + Sync,
    Key: KeyBounds,
    Value: ValueBounds<Key>,
{
    async fn init<T>(args: T) -> Self;
    fn handle_action(
        &mut self,
        action: ActionType<Key, Value>
    ) -> ImmediateValuePromise<ActionResponse<Key, Value>> {
        let action_future = match &action {
            ActionType::Update(value) => to_boxed(self.update(value)),
            ActionType::UpdateMany(values) => to_boxed(self.update_many(values)),
            ActionType::Delete(key) => to_boxed(self.delete(key)),
            ActionType::DeleteMany(values) => to_boxed(self.delete_many(values)),
        };
        ImmediateValuePromise::new(async move {
            let action_result = action_future.await;
            Ok(ActionResponse::from_type_and_result(action, action_result))
        })
    }
    fn update(&mut self, value: &Value) -> impl StorageFuture<ActionResult>;
    fn update_many(&mut self, values: &Vec<Value>) -> impl StorageFuture<ActionResult>;
    fn delete(&mut self, key: &Key) -> impl StorageFuture<ActionResult>;
    fn delete_many(&mut self, keys: &Vec<Key>) -> impl StorageFuture<ActionResult>;
}

pub trait StorageFuture<FutureOutput>
where
    Self: Future<Output = FutureOutput> + Send + Sync + 'static,
    FutureOutput: Clone + Send + Sync,
{
}

impl<T, FutOutput> StorageFuture<FutOutput> for T
where
    T: Future<Output = FutOutput> + Send + Sync + 'static,
    FutOutput: Clone + Send + Sync,
{
}

fn to_boxed<FutOutput>(fut: impl StorageFuture<FutOutput>) -> BoxFuture<'static, FutOutput>
where
    FutOutput: Clone + Send + Sync + 'static,
{
    Box::pin(fut) as BoxFuture<'static, FutOutput>
}
