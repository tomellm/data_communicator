
use itertools::Itertools;
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

pub trait DrainIf<T> {
    fn drain_if_iter<F: FnMut(&mut T) -> bool>(&mut self, pred: F) -> impl Iterator<Item = T>;
    fn drain_if<F: FnMut(&mut T) -> bool>(&mut self, pred: F) -> Vec<T>;
}

impl<T> DrainIf<T> for Vec<T> {
    fn drain_if_iter<F: FnMut(&mut T) -> bool>(&mut self, mut pred: F) -> impl Iterator<Item = T> {
        let mut indexes = self
            .iter_mut()
            .enumerate()
            .filter_map(
                |(i, ac)| {
                    if pred(ac) {
                        Some((i, ac))
                    } else {
                        None
                    }
                },
            )
            .map(|(i, _)| i)
            .collect_vec();
        indexes.sort();
        indexes.reverse();

        indexes.into_iter().map(|index| self.remove(index))
    }
    fn drain_if<F: FnMut(&mut T) -> bool>(&mut self, pred: F) -> Vec<T> {
        self.drain_if_iter(pred).collect_vec()
    }
}

