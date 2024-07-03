use lazy_async_promise::{DirectCacheAccess, ImmediateValuePromise, ImmediateValueState};

pub trait PromiseUtilities<T> {
    fn poll_and_check_finished(&mut self) -> bool;
    fn take_expect(&mut self) -> T;
}

impl<T> PromiseUtilities<T> for ImmediateValuePromise<T>
where
    T: Send + 'static,
{
    fn poll_and_check_finished(&mut self) -> bool {
        !matches!(self.poll_state(), ImmediateValueState::Updating)
    }
    fn take_expect(&mut self) -> T {
        self.take_value().unwrap()
    }
}
