use std::hash::Hash;

pub mod change;
pub mod communicator;
pub mod container;
pub mod data;
pub mod query;
pub mod storage;
pub mod test_impl;
pub mod utils;

pub trait GetKey<Key> {
    fn key(&self) -> &Key;
}

pub trait KeyBounds
where
    Self: Eq + Hash + Clone + Send + Sync + 'static,
{
}

impl<T> KeyBounds for T where T: Eq + Hash + Clone + Send + Sync + 'static {}

pub trait ValueBounds<Key>
where
    Self: Clone + GetKey<Key> + Send + Sync + 'static,
{
}

impl<T, Key> ValueBounds<Key> for T
where
    T: Clone + GetKey<Key> + Send + Sync + 'static,
    Key: Eq + Hash + Clone + Send + Sync + 'static,
{
}

pub trait BlankOutError<Value, Error> {
    fn blank_err(self) -> Result<Value, ()>;
}

impl<Value, Error> BlankOutError<Value, Error> for Result<Value, Error> {
    fn blank_err(self) -> Result<Value, ()> {
        self.or(Err(()))
    }
}

mod test {

    use std::{collections::HashMap, sync::Arc, thread, time::Duration};

    use lazy_async_promise::ImmediateValueState;
    use tokio::sync::{Mutex, MutexGuard};
    use tracing::Level;
    use uuid::Uuid;

    use super::{
        container::DataContainer,
        test_impl::test::{Item, TestStruct},
        utils::PromiseUtils,
    };

    async fn setup_container() -> (
        DataContainer<Uuid, Item, TestStruct>,
        Arc<Mutex<HashMap<Uuid, Item>>>,
    ) {
        let map = Arc::new(Mutex::new(HashMap::new()));
        (
            DataContainer::<Uuid, Item, TestStruct>::new(map.clone()).await,
            map,
        )
    }

    async fn check<T>(
        map: &Arc<Mutex<HashMap<Uuid, Item>>>,
        check_fn: impl FnOnce(&MutexGuard<HashMap<Uuid, Item>>) -> T,
    ) -> T {
        let lock = map.lock().await;
        check_fn(&lock)
    }

    #[tokio::test]
    async fn simple_test() {
        tracing_subscriber::fmt::fmt()
            .with_max_level(Level::TRACE)
            .init();

        let (mut container, data_map) = setup_container().await;
        let mut communicator_one = container.communicator();
        let new_item = Item::new("some_value");

        println!("about to assert that map is empty");
        assert!(check(&data_map, |map| map.is_empty()).await);
        println!("map was empty");

        let _promise = communicator_one.update(new_item.clone());
        thread::sleep(Duration::from_secs(1));

        communicator_one.state_update();
        container.state_update();

        println!("about to do second check");
        assert_eq!(
            check(&data_map, |map| map.get(&new_item.uuid).cloned()).await,
            Some(new_item)
        );
        println!("finished second check");
    }
}
