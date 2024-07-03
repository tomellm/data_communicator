use lazy_async_promise::{DirectCacheAccess, ImmediateValuePromise, ImmediateValueState};


pub trait PromiseUtils<T> {
    fn poll_and_check_finished(&mut self) -> bool;
    fn take_expect(&mut self) -> T;
}

impl<T> PromiseUtils<T> for ImmediateValuePromise<T>
where
    T: Send + 'static
{
    fn poll_and_check_finished(&mut self) -> bool {
        match self.poll_state() {
            ImmediateValueState::Updating => false,
            _ => true 
        }
    }
    fn take_expect(&mut self) -> T {
        self.take_value().unwrap()
    }
}
