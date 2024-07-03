use std::{
    collections::HashMap,
    ops::{Deref, DerefMut},
};

use super::{change::ChangeType, KeyBounds, ValueBounds};

pub struct Data<Key, Value>
where
    Key: KeyBounds,
    Value: ValueBounds<Key>,
{
    data: HashMap<Key, Value>,
}

impl<Key, Value> Data<Key, Value>
where
    Key: KeyBounds,
    Value: ValueBounds<Key>,
{
    #[must_use]
    pub fn new() -> Self {
        Self {
            data: HashMap::new(),
        }
    }

    pub fn update_with_fresh(&mut self, fresh_data: FreshData<Key, Value>) {
        self.data.extend(HashMap::from(fresh_data));
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
            Self::Update(values) => Self::update(values, data),
            Self::Delete(keys) => Self::delete(keys, data),
        }
    }

    fn update(values: Vec<Value>, data: &mut Data<Key, Value>) {
        for value in values {
            data.data.insert(value.key().clone(), value);
        }
    }
    fn delete(keys: Vec<Key>, data: &mut Data<Key, Value>) {
        for key in keys {
            data.data.remove(&key);
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
