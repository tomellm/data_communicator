pub mod test {
    #![allow(private_interfaces)]
    use std::collections::HashMap;
    use std::sync::Arc;

    use itertools::Itertools;
    use tokio::sync::Mutex;
    use uuid::Uuid;

    use crate::buffered::query::Predicate;

    use super::super::{
        change::ChangeResult,
        query::{QueryError, QueryResponse},
        storage::{Future, Storage},
        GetKey,
    };

    #[derive(Clone, PartialEq, Eq, Debug)]
    pub struct Item {
        pub uuid: Uuid,
        pub val: String,
    }

    impl Item {
        #[must_use]
        pub fn new(val: &str) -> Self {
            Self {
                uuid: Uuid::new_v4(),
                val: String::from(val),
            }
        }
    }

    impl GetKey<Uuid> for Item {
        fn key(&self) -> &Uuid {
            &self.uuid
        }
    }

    pub struct ExampleStorage {
        pub map: Arc<Mutex<HashMap<Uuid, Item>>>,
    }

    impl Storage<Uuid, Item> for ExampleStorage {
        type InitArgs = Arc<Mutex<HashMap<Uuid, Item>>>;
        async fn init(args: Self::InitArgs) -> Self {
            ExampleStorage { map: args }
        }
        fn update_many(&mut self, values: &[Item]) -> impl Future<ChangeResult> {
            let tuples = values.iter().cloned().map(|i| (*i.key(), i)).collect_vec();
            let map = self.map.clone();
            async move {
                map.lock().await.extend(tuples);
                ChangeResult::Success
            }
        }
        fn delete(&mut self, key: &Uuid) -> impl Future<ChangeResult> {
            let map = self.map.clone();
            let key = *key;
            async move {
                map.lock().await.remove(&key);
                ChangeResult::Success
            }
        }
        fn delete_many(&mut self, keys: &[Uuid]) -> impl Future<ChangeResult> {
            let map = self.map.clone();
            let keys = keys.to_vec();
            async move {
                let _ = map.lock().await.extract_if(|key, _| keys.contains(key));
                ChangeResult::Success
            }
        }
        fn update(&mut self, value: &Item) -> impl Future<ChangeResult> {
            let map = self.map.clone();
            let item = value.clone();
            async move {
                map.lock().await.insert(*item.key(), item);
                ChangeResult::Success
            }
        }
        fn get_by_id(&mut self, key: Uuid) -> impl Future<QueryResponse<Uuid, Item>> {
            let map = self.map.clone();
            async move {
                map.lock()
                    .await
                    .get(&key)
                    .map_or(QueryResponse::Err(QueryError::NotPresent), |item| {
                        QueryResponse::Ok(item.clone().into())
                    })
            }
        }
        fn get_by_ids(&mut self, keys: Vec<Uuid>) -> impl Future<QueryResponse<Uuid, Item>> {
            let map = self.map.clone();
            async move {
                QueryResponse::Ok(
                    map.lock()
                        .await
                        .iter()
                        .filter(|(k, _)| keys.contains(k))
                        .map(|(k, v)| (*k, v.clone()))
                        .collect::<HashMap<_, _>>()
                        .into(),
                )
            }
        }
        fn get_by_predicate(
            &mut self,
            predicate: Predicate<Item>,
        ) -> impl Future<QueryResponse<Uuid, Item>> {
            let map = self.map.clone();
            async move {
                QueryResponse::Ok(
                    map.lock()
                        .await
                        .iter()
                        .filter(|(_, v)| predicate(v))
                        .map(|(k, v)| (*k, v.clone()))
                        .collect::<HashMap<_, _>>()
                        .into(),
                )
            }
        }
    }
}
