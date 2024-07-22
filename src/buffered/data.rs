use std::{
    cmp::Ordering,
    collections::HashMap,
    ops::{Deref, DerefMut},
};

use itertools::Itertools;
use permutation::Permutation;

use super::{change::ChangeType, KeyBounds, ValueBounds};

pub struct Data<Key, Value>
where
    Key: KeyBounds,
    Value: ValueBounds<Key>,
{
    pub data: HashMap<Key, Value>,
    pub sorted: Permutation,
    pub sorting_fn: Box<dyn FnMut(&Value, &Value) -> Ordering>,
}

impl<Key, Value> Data<Key, Value>
where
    Key: KeyBounds,
    Value: ValueBounds<Key>,
{
    #[must_use]
    pub fn new() -> Self {
        let data = HashMap::new();
        let sorting_fn = |a: &Value, b: &Value| a.key().cmp(&b.key());
        Self {
            data,
            sorted: permutation::sort_by(Vec::<Value>::new(), sorting_fn),
            sorting_fn: Box::new(sorting_fn),
        }
    }
    pub fn extend(&mut self, extend: HashMap<Key, Value>) {
        self.data.extend(extend);
        self.resort();
    }
    pub fn update(&mut self, update: Vec<Value>) {
        self.data
            .extend(update.into_iter().map(|v| (v.key().clone(), v)));
        self.resort();
    }
    pub fn delete(&mut self, keys: Vec<Key>) {
        let _ = self.data.extract_if(|k, _| keys.contains(k));
        self.resort();
    }
    pub fn resort(&mut self) {
        self.sorted = permutation::sort_by(&self.data.values().collect_vec(), |a, b| {
            (self.sorting_fn)(*a, *b)
        })
    }
    pub fn new_sorting_fn<F: FnMut(&Value, &Value) -> Ordering + 'static>(&mut self, sorting_fn: F) {
        self.sorting_fn = Box::new(sorting_fn);
        self.resort();
    }
}

impl<Key, Value> Default for Data<Key, Value>
where
    Key: KeyBounds,
    Value: ValueBounds<Key>,
{
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone)]
pub struct FreshData<Key, Value>(HashMap<Key, Value>);

impl<Key, Value> FreshData<Key, Value>
where
    Key: KeyBounds,
    Value: ValueBounds<Key>,
{
    pub fn add_fresh_data(self, data: &mut Data<Key, Value>) {
        data.extend(HashMap::from(self));
    }
}

impl<Key, Value> Deref for FreshData<Key, Value> {
    type Target = HashMap<Key, Value>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<Key, Value> DerefMut for FreshData<Key, Value> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<Key, Value> From<Value> for FreshData<Key, Value>
where
    Key: KeyBounds,
    Value: ValueBounds<Key>,
{
    fn from(value: Value) -> Self {
        let mut map = HashMap::new();
        map.insert(value.key().clone(), value);
        FreshData(map)
    }
}

impl<Key, Value> From<Vec<Value>> for FreshData<Key, Value>
where
    Key: KeyBounds,
    Value: ValueBounds<Key>,
{
    fn from(value: Vec<Value>) -> Self {
        FreshData(
            value
                .into_iter()
                .map(|item| (item.key().clone(), item))
                .collect(),
        )
    }
}

impl<Key, Value> From<HashMap<Key, Value>> for FreshData<Key, Value>
where
    Key: KeyBounds,
    Value: ValueBounds<Key>,
{
    fn from(value: HashMap<Key, Value>) -> Self {
        Self(value)
    }
}

#[allow(clippy::implicit_hasher)]
impl<Key, Value> From<FreshData<Key, Value>> for HashMap<Key, Value>
where
    Key: KeyBounds,
    Value: ValueBounds<Key>,
{
    fn from(value: FreshData<Key, Value>) -> Self {
        value.0
    }
}

#[derive(Clone)]
pub enum DataChange<Key, Value>
where
    Key: KeyBounds,
    Value: ValueBounds<Key>,
{
    Update(Vec<Value>),
    Delete(Vec<Key>),
}

impl<Key, Value> DataChange<Key, Value>
where
    Key: KeyBounds,
    Value: ValueBounds<Key>,
{
    pub fn value_keys(&self) -> Vec<&Key> {
        match self {
            Self::Update(values) => values.iter().map(Value::key).collect::<Vec<_>>(),
            Self::Delete(keys) => keys.iter().collect::<Vec<_>>(),
        }
    }

    /// Internally decides how the data is mutated depending in the data update
    /// state
    pub fn update_data(self, data: &mut Data<Key, Value>) {
        match self {
            Self::Update(values) => data.update(values),
            Self::Delete(keys) => data.delete(keys),
        }
    }
}

impl<Key, Value> From<ChangeType<Key, Value>> for DataChange<Key, Value>
where
    Key: KeyBounds,
    Value: ValueBounds<Key>,
{
    fn from(value: ChangeType<Key, Value>) -> Self {
        match value {
            ChangeType::Update(val) => Self::Update(vec![val]),
            ChangeType::UpdateMany(vals) => Self::Update(vals),
            ChangeType::Delete(key) => Self::Delete(vec![key]),
            ChangeType::DeleteMany(keys) => Self::Delete(keys),
        }
    }
}
