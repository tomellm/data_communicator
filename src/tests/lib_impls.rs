use std::collections::HashMap;

use itertools::Itertools;

use crate::{
    change::ChangeResult, container::
        storage::{Future, InitFuture, Storage},
     query::{Predicate, QueryError, QueryResponse}, GetKey
};

impl GetKey<usize> for TestStruct {
    fn key(&self) -> &usize {
        &self.key
    }
}

#[derive(Clone)]
pub(super) struct TestStruct {
    pub(super) key: usize,
    pub(super) val: String,
}

impl Storage<usize, TestStruct> for HashMap<usize, TestStruct> {
    type InitArgs = ();

    fn init(_: Self::InitArgs) -> impl InitFuture<Self> {
        async move { HashMap::new() }
    }

    fn insert(&mut self, value: &TestStruct) -> impl Future<ChangeResult> {
        self.insert(*value.key(), value.clone());
        async move { ChangeResult::Success }
    }

    fn insert_many(&mut self, values: &[TestStruct]) -> impl Future<ChangeResult> {
        self.extend(values.iter().map(|v| (v.key, v.clone())));
        async move { ChangeResult::Success }
    }

    fn update(&mut self, value: &TestStruct) -> impl Future<ChangeResult> {
        if let Some(val) = self.get_mut(&value.key) {
            val.val = value.val.clone();
        }
        async move { ChangeResult::Success }
    }

    fn update_many(&mut self, values: &[TestStruct]) -> impl Future<ChangeResult> {
        for value in values {
            if let Some(val) = self.get_mut(&value.key) {
                val.val = value.val.clone();
            }
        }
        async move { ChangeResult::Success }
    }

    fn delete(&mut self, key: &usize) -> impl Future<ChangeResult> {
        self.remove(key);
        async move { ChangeResult::Success }
    }

    fn delete_many(&mut self, keys: &[usize]) -> impl Future<ChangeResult> {
        for key in keys {
            self.remove(key);
        }
        async move { ChangeResult::Success }
    }

    fn get_all(&mut self) -> impl Future<QueryResponse<usize, TestStruct>> {
        let values = self.clone();
        async move { QueryResponse::Ok(values.into()) }
    }

    fn get_by_id(&mut self, key: usize) -> impl Future<QueryResponse<usize, TestStruct>> {
        let res = self
            .get(&key)
            .map(|val| QueryResponse::Ok(val.clone().into()))
            .unwrap_or(QueryResponse::Err(QueryError::NotPresent));
        async move { res }
    }

    fn get_by_ids(&mut self, keys: Vec<usize>) -> impl Future<QueryResponse<usize, TestStruct>> {
        let mut vals = vec![];
        let mut err = None;
        for key in keys {
            if let Some(val) = self.get(&key) {
                vals.push(val.clone());
            } else {
                err = Some(QueryResponse::Err(QueryError::NotPresent));
                break;
            }
        }
        async move { err.unwrap_or(QueryResponse::Ok(vals.into())) }
    }

    fn get_by_predicate(
        &mut self,
        predicate: Predicate<TestStruct>,
    ) -> impl Future<QueryResponse<usize, TestStruct>> {
        let values = self
            .values()
            .filter(|val| predicate(val))
            .cloned()
            .collect_vec();
        async move { QueryResponse::Ok(values.into()) }
    }
}
