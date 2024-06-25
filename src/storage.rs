//! hello

use futures::{future::BoxFuture, Future};
use lazy_async_promise::ImmediateValuePromise;

use super::changer::{self, ActionType, Response};

pub trait GetKey<Key> {
    fn get_key(&self) -> Key;
}

pub trait StorageFuture<FutOutput>
where
    Self: Future<Output = FutOutput> + Send + Sync + 'static,
    FutOutput: Clone + Send + Sync,
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

pub trait Storage<Key, Value>
where
    Self: Send + Sync,
    Key: Clone + Send + Sync + 'static,
    Value: GetKey<Key> + Clone + Send + Sync + 'static,
{
    fn handle_action(
        &mut self,
        action: changer::Action<Key, Value>,
    ) -> ImmediateValuePromise<Response<Key, Value>> {
        println!("got a action about to handle it");
        let (action, responder) = action.get();
        let future = match action {
            ActionType::Set(value) => to_boxed(self.set(value)),
            ActionType::SetMany(values) => to_boxed(self.set_many(values)),
            ActionType::Update(value) => to_boxed(self.update(value)),
            ActionType::UpdateMany(values) => to_boxed(self.update_many(values)),
            ActionType::Delete(key) => to_boxed(self.delete(key)),
            ActionType::DeleteMany(values) => to_boxed(self.delete_many(values)),
            ActionType::GetAll(_) => to_boxed(self.get_all()),
        };
        ImmediateValuePromise::new(async move {
            let response = future.await;
            let _ = responder.send(response.clone());
            Ok(response)
        })
    }
    fn set(&mut self, value: Value) -> impl StorageFuture<Response<Key, Value>>;
    fn set_many(&mut self, values: Vec<Value>) -> impl StorageFuture<Response<Key, Value>>;
    fn update(&mut self, value: Value) -> impl StorageFuture<Response<Key, Value>>;
    fn update_many(&mut self, values: Vec<Value>) -> impl StorageFuture<Response<Key, Value>>;
    fn delete(&mut self, key: Key) -> impl StorageFuture<Response<Key, Value>>;
    fn delete_many(&mut self, keys: Vec<Key>) -> impl StorageFuture<Response<Key, Value>>;
    fn get_all(&mut self) -> impl StorageFuture<Response<Key, Value>>;
    fn setup<SetupState>(
        &mut self,
        state: SetupState,
    ) -> impl StorageFuture<Result<Vec<Value>, ()>>;
}

#[cfg(test)]
mod tests {
    use std::{collections::HashMap, sync::Arc};

    use tokio::sync::Mutex;

    use super::*;

    impl GetKey<char> for String {
        fn get_key(&self) -> char {
            self.chars().nth(0).unwrap()
        }
    }

    impl GetKey<char> for &str {
        fn get_key(&self) -> char {
            String::from(*self).get_key()
        }
    }

    struct MyStorage {
        pub map: Arc<Mutex<HashMap<char, String>>>,
    }

    impl Storage<char, String> for MyStorage {
        fn setup<SomeState>(
            &mut self,
            _state: SomeState,
        ) -> impl StorageFuture<Result<Vec<String>, ()>> {
            async move { Ok(vec![]) as Result<Vec<String>, ()> }
        }
        fn set(&mut self, value: String) -> impl StorageFuture<Response<char, String>> {
            let map = self.map.clone();
            async move {
                map.lock().await.insert(value.get_key(), value.clone());
                Response::ok(&ActionType::Set(value))
            }
        }
        fn set_many(&mut self, values: Vec<String>) -> impl StorageFuture<Response<char, String>> {
            let map = self.map.clone();
            async move {
                let mut guard = map.lock().await;
                for val in values.clone() {
                    guard.insert(val.get_key(), val);
                }
                Response::ok(&ActionType::SetMany(values))
            }
        }
        fn update(&mut self, value: String) -> impl StorageFuture<Response<char, String>> {
            let map = self.map.clone();
            async move {
                map.lock().await.insert(value.get_key(), value.clone());
                Response::ok(&ActionType::Update(value))
            }
        }
        fn update_many(
            &mut self,
            values: Vec<String>,
        ) -> impl StorageFuture<Response<char, String>> {
            let map = self.map.clone();
            async move {
                let mut guard = map.lock().await;
                for val in values.clone() {
                    guard.insert(val.get_key(), val.clone());
                }
                Response::ok(&ActionType::UpdateMany(values))
            }
        }
        fn delete(&mut self, key: char) -> impl StorageFuture<Response<char, String>> {
            let map = self.map.clone();
            async move {
                map.lock().await.remove(&key);
                Response::ok(&ActionType::Delete(key))
            }
        }
        fn delete_many(&mut self, keys: Vec<char>) -> impl StorageFuture<Response<char, String>> {
            let map = self.map.clone();
            async move {
                let mut guard = map.lock().await;
                for key in keys.iter() {
                    guard.remove(key);
                }
                Response::ok(&ActionType::DeleteMany(keys))
            }
        }
        fn get_all(&mut self) -> impl StorageFuture<Response<char, String>> {
            let map = self.map.clone();
            async move {
                let vals = map
                    .lock()
                    .await
                    .values()
                    .into_iter()
                    .map(|v| v.clone())
                    .collect::<Vec<_>>();
                Response::ok(&ActionType::GetAll(vals))
            }
        }
    }

    fn expected_hashmap(vals: Vec<&str>) -> HashMap<char, String> {
        let mut values = vals
            .into_iter()
            .map(|v| {
                let str_val = String::from(v);
                (v.get_key(), str_val)
            })
            .collect::<Vec<_>>();
        values.push((ORIGINAL_ONE.get_key(), String::from(ORIGINAL_ONE)));
        values.push((ORIGINAL_TWO.get_key(), String::from(ORIGINAL_TWO)));
        values.into_iter().collect::<HashMap<_, _>>()
    }

    const ORIGINAL_ONE: &str = "z value one";
    const ORIGINAL_TWO: &str = "y other value";

    fn setup_storage() -> MyStorage {
        let mut map = HashMap::new();
        map.insert(ORIGINAL_ONE.get_key(), String::from(ORIGINAL_ONE));
        map.insert(ORIGINAL_TWO.get_key(), String::from(ORIGINAL_TWO));

        MyStorage {
            map: Arc::new(Mutex::new(map)),
        }
    }

    #[tokio::test]
    async fn check_set_futures_are_being_awaited() {
        let mut storage = setup_storage();

        let value_one = "awhat is this value";
        let _ = storage.set(String::from(value_one)).await;

        let value_two = "b first value";
        let value_three = "c second value";
        let _ = storage
            .set_many(vec![String::from(value_three), String::from(value_two)])
            .await;

        let map_guard = storage.map.lock().await;
        assert_eq!(
            *map_guard,
            expected_hashmap(vec![value_one, value_two, value_three])
        );
    }

    #[tokio::test]
    async fn check_update_futures_are_beeing_awaited() {
        let mut storage = setup_storage();

        let value_one = "a what is this value";
        let _ = storage.update(String::from(value_one)).await;

        let value_two = "b first value";
        let value_three = "c second value";
        let _ = storage
            .update_many(vec![String::from(value_two), String::from(value_three)])
            .await;

        let map_guard = storage.map.lock().await;
        assert_eq!(
            *map_guard,
            expected_hashmap(vec![value_one, value_two, value_three])
        );
    }

    #[tokio::test]
    async fn check_delete_futures_are_beeing_awaited() {
        let mut storage = setup_storage();

        let value_one = "a some new value";
        let _ = storage.set(String::from(value_one)).await;
        let _ = storage.delete(value_one.get_key()).await;

        let _ = storage
            .delete_many(vec![ORIGINAL_ONE.get_key(), ORIGINAL_TWO.get_key()])
            .await;

        let map_guard = storage.map.lock().await;
        assert_eq!(*map_guard, HashMap::new());
    }
}
