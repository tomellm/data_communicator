#![feature(type_alias_impl_trait)]
#![feature(impl_trait_in_fn_trait_return)]

use std::{fmt::Debug, hash::Hash};

use itertools::Itertools;

pub mod change;
pub mod communicator;
pub mod container;
pub mod query;
pub mod utils;
#[cfg(test)]
mod tests;

pub trait GetKey<Key> {
    fn key(&self) -> &Key;
}

impl<Key> GetKey<Key> for Key {
    fn key(&self) -> &Key {
        self
    }
}

pub trait GetKeys<Key> {
    fn keys(&self) -> Vec<&Key>;
}

impl<V, Key> GetKeys<Key> for Vec<V>
where
    V: GetKey<Key>
{
    fn keys(&self) -> Vec<&Key> {
        self.iter().map(GetKey::key).collect_vec()
    }
}

impl<V, Key> GetKeys<Key> for &Vec<V>
where
    V: GetKey<Key>
{
    fn keys(&self) -> Vec<&Key> {
        self.iter().map(GetKey::key).collect_vec()
    }
}

pub trait KeyBounds
where
    Self: Debug + Ord + Eq + Hash + Clone + Send + Sync + 'static,
{
}

impl<T> KeyBounds for T where T: Debug + Ord + Eq + Hash + Clone + Send + Sync + 'static {}

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


