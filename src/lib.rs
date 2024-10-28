//! A crate made for async direct render Applications to access, share and modify 
//! data with a single point of truth.
//!
//! ### Use Case
//!
//! I created this lib while working on a egui application. My use case was that 
//! many different parts of my application had to access the same data in a database,
//! make changes to the data and also recive new changes from other parts of the
//! application. 
//!
//! Other then the main reason mentioned above I also wanted to avoid directly 
//! calling database code from application code and dealing with `Arc<Mutex<_>>`.
//!
//! ### Key Information
//! - [`DataContainer`][crate::container::DataContainer] is the orchestrator and
//!     the one that calls the actual database.
//! - The "Database" is actually just an implementation of the [`Storage`][crate::container::storage::Storage]
//!     trait.
//! - `state_update` is what will make both communicators and the container recive
//!     and send values meaning it has to be called periodically.
//! - [`Communicator`][crate::communicator::Communicator]'s can query to change
//!     the data they are viewing.
//! - [`Communicator`][crate::communicator::Communicator]'s can ask for changes
//!     by [`insert`][crate::communicator::Communicator::insert], [`update`][crate::communicator::Communicator::update] and [`delete`][crate::communicator::Communicator::delete]
//!     and the respecitive `..._many` variants.
//! - The `Key` and `Value` pair you choose have to match the [`KeyBounds`] and [`ValueBounds`]
//!     respecively. But most noteably the `Value` needs to implement the [`GetKey`] trait.
//!
//! ### Example
//!
//! Although it is quite hard to give a quick example below I have tried to give
//! an idea on how I use this library. During the setup we initialize the
//! [`DataContainer`][crate::container::DataContainer]
//! of our data and then create as many [`Communicator`][crate::communicator::Communicator]'s 
//! as needed. Consider that the communicators are not `Clone` since the container
//! needs to know of them meaning they cannot simply be cloned.
//!
//! > *IMPORTANT!!!*: Both the [`container`][crate::container::DataContainer::state_update]
//! > as well as the [`communicator`][crate::communicator::Communicator::state_update]
//! > need to have the `state_update` function called to work. If not no queries
//! > or changes will be recived or returned.
//! ```
//! let container = DataContainer::init().await
//!
//! let comm_1 = container.communicator();
//! let comm_2 = container.communicator();
//!
//! let app_a = AppA::init(comm_1).await;
//! let app_b = AppB::init(comm_2).await;
//!
//! // -- Application Loop
//! loop {
//!     container.state_update(); // <-- ***IMPORTANT***
//!     match current_app {
//!         AppA => app_a.view(),
//!         AppB => app_b.view()
//!     }
//! }
//! ```
//! Personally I like defining the data the App is interested in at startup using
//! a [`query`][crate::communicator::Communicator::query] so that I have all the
//! information directly to begin with. Although this can always be changed later.
//!
//! All actions unfortunatly return a future that will resolve to the status of the
//! action. For more information about this check out [`QueryResult`][crate::query::QueryResult]
//! and [`ChangeResult`][crate::change::ChangeResult] respecively. The actual 
//! query or change result will automatically take place for the communicator.
//! ```
//! impl AppA {
//!     fn init(comm: Communicator<Key, Value>) -> Future<Output = Self> + Send + 'static {
//!         async move {
//!             let _ = comm.query(QueryType::All).await;
//!             Self { comm, .. }
//!         }
//!     }
//!     fn view(&mut self) {
//!         self.comm.state_update(); // <-- ***IMPORTANT***
//!
//!         if button("insert").clicked() {
//!             let future = self.comm.insert(MyStruct {});
//!             // -- await the future in some way
//!         }
//!     }
//! }
//! ```

#![allow(clippy::manual_async_fn)]
#![feature(type_alias_impl_trait)]
#![feature(impl_trait_in_fn_trait_return)]

use std::{fmt::Debug, hash::Hash};

use itertools::Itertools;

pub mod change;
pub mod communicator;
pub mod container;
pub mod query;
mod utils;
#[cfg(test)]
mod tests;


/// This trait simplifies all interactions since it defines how to get the `Key`
/// out of the `Value` type.
pub trait GetKey<Key> {
    fn key(&self) -> &Key;
}

impl<Key> GetKey<Key> for Key {
    fn key(&self) -> &Key {
        self
    }
}


pub(crate) trait GetKeys<Key> {
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

/// The `Trait`'s the `Key` type needs to implement
///
/// ```
/// Self: Debug + Ord + Eq + Hash + Clone + Send + Sync + 'static,
/// ```
pub trait KeyBounds
where
    Self: Debug + Ord + Eq + Hash + Clone + Send + Sync + 'static,
{
}

impl<T> KeyBounds for T where T: Debug + Ord + Eq + Hash + Clone + Send + Sync + 'static {}

/// The `Trait`'s the `Value` type needs to implement
///
/// ```
/// Self: Clone + GetKey<Key> + Send + Sync + 'static
/// ```
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
