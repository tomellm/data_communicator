use std::hash::Hash;

pub mod change;
pub mod communicator;
pub mod container;
pub mod data;
pub mod query;
pub mod storage;
pub mod utils;

pub trait GetKey<Key> {
    fn key(&self) -> &Key;
}

pub trait KeyBounds
where
    Self: Ord + Eq + Hash + Clone + Send + Sync + 'static,
{
}

impl<T> KeyBounds for T where T: Ord + Eq + Hash + Clone + Send + Sync + 'static {}

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
