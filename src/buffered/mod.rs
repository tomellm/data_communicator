use std::hash::Hash;

pub mod container;
pub mod communicator;
pub mod actions;
pub mod responses;
pub mod storage;
pub mod data;

pub trait GetKey<Key> {
    fn key(&self) -> &Key;
}

pub trait KeyBounds
where
    Self: Eq + Hash + Clone + Send + Sync + 'static {
}

pub trait ValueBounds<Key>
where
    Self: Clone + GetKey<Key> + Send + Sync + 'static {

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

    #[tokio::test]
    fn simple_test() {

    }
}
