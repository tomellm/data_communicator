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
    #![allow(dead_code)]

    use std::{collections::HashMap, sync::Arc, thread, time::Duration};

    use lazy_async_promise::{ImmediateValuePromise, ImmediateValueState};
    use tokio::sync::{Mutex, MutexGuard};
    #[allow(unused_imports)]
    use tracing::Level;
    use tracing::{debug, info, trace, warn};
    use uuid::Uuid;

    use super::{
        communicator::Communicator,
        container::DataContainer,
        query::QueryType,
        test_impl::test::{ExampleStorage, Item},
    };

    async fn setup_container() -> (
        DataContainer<Uuid, Item, ExampleStorage>,
        Arc<Mutex<HashMap<Uuid, Item>>>,
    ) {
        let map = Arc::new(Mutex::new(HashMap::new()));
        (
            DataContainer::<Uuid, Item, ExampleStorage>::new(map.clone()).await,
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

    fn await_promise<T>(
        container: &mut DataContainer<Uuid, Item, ExampleStorage>,
        communicator: &mut Communicator<Uuid, Item>,
        mut promise: ImmediateValuePromise<T>,
    ) -> ImmediateValuePromise<T>
    where
        T: Send + Sync + 'static,
    {
        loop {
            container.state_update();
            communicator.state_update();
            match promise.poll_state() {
                ImmediateValueState::Updating => continue,
                ImmediateValueState::Empty => {
                    warn!("Promise was empty, should this state happen???");
                    break;
                }
                _ => {
                    info!("Promise finished, exiting loop!");
                    break
                },
            }
        }
        promise
    }

    #[tokio::test]
    async fn simple_test() {
        tracing_subscriber::fmt::fmt()
            .with_max_level(Level::TRACE)
            .init();

        let (mut container, data_map) = setup_container().await;
        let mut communicator_one = container.communicator();
        let new_item = Item::new("some_value");

        info!("about to assert that map is empty");
        assert!(check(&data_map, |map| map.is_empty()).await);
        info!("map was empty");

        let get_promise = communicator_one.query(QueryType::GetById(new_item.uuid));
        let _get_promise = await_promise(&mut container, &mut communicator_one, get_promise);

        let update_promise = communicator_one.update(new_item.clone());
        let _update_promise = await_promise(&mut container, &mut communicator_one, update_promise);

        info!("about to do second check");
        assert_eq!(
            check(&data_map, |map| map.get(&new_item.uuid).cloned()).await,
            Some(new_item)
        );
        println!("finished second check");
    }
}
