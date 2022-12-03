use borsh::{BorshDeserialize, BorshSerialize};

use super::iter::{Iter, IterMut};
use super::{ChunkedVector, ERR_INDEX_OUT_OF_BOUNDS};
use near_sdk::env;

impl<'a, T, const N: usize> IntoIterator for &'a ChunkedVector<T, N>
where
    T: BorshSerialize + BorshDeserialize,
{
    type Item = &'a T;
    type IntoIter = Iter<'a, T, N>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<'a, T, const N: usize> IntoIterator for &'a mut ChunkedVector<T, N>
where
    T: BorshSerialize + BorshDeserialize,
{
    type Item = &'a mut T;
    type IntoIter = IterMut<'a, T, N>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter_mut()
    }
}

impl<T, const N: usize> Extend<T> for ChunkedVector<T, N>
where
    T: BorshSerialize + BorshDeserialize,
{
    fn extend<I>(&mut self, iter: I)
    where
        I: IntoIterator<Item = T>,
    {
        for item in iter {
            self.push(item)
        }
    }
}

impl<T, const N: usize> core::ops::Index<u32> for ChunkedVector<T, N>
where
    T: BorshSerialize + BorshDeserialize,
{
    type Output = T;

    fn index(&self, index: u32) -> &Self::Output {
        self.get(index)
            .unwrap_or_else(|| env::panic_str(ERR_INDEX_OUT_OF_BOUNDS))
    }
}

impl<T, const N: usize> core::ops::IndexMut<u32> for ChunkedVector<T, N>
where
    T: BorshSerialize + BorshDeserialize,
{
    fn index_mut(&mut self, index: u32) -> &mut Self::Output {
        self.get_mut(index)
            .unwrap_or_else(|| env::panic_str(ERR_INDEX_OUT_OF_BOUNDS))
    }
}
