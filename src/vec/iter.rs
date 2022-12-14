use borsh::{BorshDeserialize, BorshSerialize};
use core::{iter::FusedIterator, ops::Range};

use super::{ChunkedVector, ERR_INDEX_OUT_OF_BOUNDS};
use near_sdk::env;

/// An iterator over references to each element in the stored vector.
#[derive(Debug)]
pub struct Iter<'a, T, const N: usize>
where
    T: BorshSerialize + BorshDeserialize,
{
    /// Underlying vector to iterate through
    vec: &'a ChunkedVector<T, N>,
    /// Range of indices to iterate.
    range: Range<u32>,
}

impl<'a, T, const N: usize> Iter<'a, T, N>
where
    T: BorshSerialize + BorshDeserialize,
{
    pub(super) fn new(vec: &'a ChunkedVector<T, N>) -> Self {
        Self {
            vec,
            range: Range {
                start: 0,
                end: vec.len(),
            },
        }
    }

    /// Returns number of elements left to iterate.
    fn remaining(&self) -> usize {
        self.range.len()
    }
}

impl<'a, T, const N: usize> Iterator for Iter<'a, T, N>
where
    T: BorshSerialize + BorshDeserialize,
{
    type Item = &'a T;

    fn next(&mut self) -> Option<Self::Item> {
        <Self as Iterator>::nth(self, 0)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.remaining();
        (remaining, Some(remaining))
    }

    fn count(self) -> usize {
        self.remaining()
    }

    fn nth(&mut self, n: usize) -> Option<Self::Item> {
        let idx = self.range.nth(n)?;
        Some(
            self.vec
                .get(idx)
                .unwrap_or_else(|| env::panic_str(ERR_INDEX_OUT_OF_BOUNDS)),
        )
    }
}

impl<'a, T, const N: usize> ExactSizeIterator for Iter<'a, T, N> where
    T: BorshSerialize + BorshDeserialize
{
}
impl<'a, T, const N: usize> FusedIterator for Iter<'a, T, N> where
    T: BorshSerialize + BorshDeserialize
{
}

impl<'a, T, const N: usize> DoubleEndedIterator for Iter<'a, T, N>
where
    T: BorshSerialize + BorshDeserialize,
{
    fn next_back(&mut self) -> Option<Self::Item> {
        <Self as DoubleEndedIterator>::nth_back(self, 0)
    }

    fn nth_back(&mut self, n: usize) -> Option<Self::Item> {
        let idx = self.range.nth_back(n)?;
        Some(
            self.vec
                .get(idx)
                .unwrap_or_else(|| env::panic_str(ERR_INDEX_OUT_OF_BOUNDS)),
        )
    }
}

/// An iterator over exclusive references to each element of a stored vector.
#[derive(Debug)]
pub struct IterMut<'a, T, const N: usize>
where
    T: BorshSerialize + BorshDeserialize,
{
    /// Mutable reference to vector used to iterate through.
    vec: &'a mut ChunkedVector<T, N>,
    /// Range of indices to iterate.
    range: Range<u32>,
}

impl<'a, T, const N: usize> IterMut<'a, T, N>
where
    T: BorshSerialize + BorshDeserialize,
{
    /// Creates a new iterator for the given storage vector.
    pub(crate) fn new(vec: &'a mut ChunkedVector<T, N>) -> Self {
        let end = vec.len();
        Self {
            vec,
            range: Range { start: 0, end },
        }
    }

    /// Returns the amount of remaining elements to yield by the iterator.
    fn remaining(&self) -> usize {
        self.range.len()
    }
}

impl<'a, T, const N: usize> IterMut<'a, T, N>
where
    T: BorshSerialize + BorshDeserialize,
{
    fn get_mut<'b>(&'b mut self, at: u32) -> Option<&'a mut T> {
        self.vec.get_mut(at).map(|value| {
            //* SAFETY: The lifetime can be swapped here because we can assert that the iterator
            //*         will only give out one mutable reference for every individual item
            //*         during the iteration, and there is no overlap. This must be checked
            //*         that no element in this iterator is ever revisited during iteration.
            unsafe { &mut *(value as *mut T) }
        })
    }
}

impl<'a, T, const N: usize> Iterator for IterMut<'a, T, N>
where
    T: BorshSerialize + BorshDeserialize,
{
    type Item = &'a mut T;

    fn next(&mut self) -> Option<Self::Item> {
        <Self as Iterator>::nth(self, 0)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.remaining();
        (remaining, Some(remaining))
    }

    fn count(self) -> usize {
        self.remaining()
    }

    fn nth(&mut self, n: usize) -> Option<Self::Item> {
        let idx = self.range.nth(n)?;
        Some(
            self.get_mut(idx)
                .unwrap_or_else(|| env::panic_str(ERR_INDEX_OUT_OF_BOUNDS)),
        )
    }
}

impl<'a, T, const N: usize> ExactSizeIterator for IterMut<'a, T, N> where
    T: BorshSerialize + BorshDeserialize
{
}
impl<'a, T, const N: usize> FusedIterator for IterMut<'a, T, N> where
    T: BorshSerialize + BorshDeserialize
{
}

impl<'a, T, const N: usize> DoubleEndedIterator for IterMut<'a, T, N>
where
    T: BorshSerialize + BorshDeserialize,
{
    fn next_back(&mut self) -> Option<Self::Item> {
        <Self as DoubleEndedIterator>::nth_back(self, 0)
    }

    fn nth_back(&mut self, n: usize) -> Option<Self::Item> {
        let idx = self.range.nth_back(n)?;
        Some(
            self.get_mut(idx)
                .unwrap_or_else(|| env::panic_str(ERR_INDEX_OUT_OF_BOUNDS)),
        )
    }
}

// TODO drain is possible, it's just complex to do efficiently
// /// A draining iterator for [`Vector<T>`].
// #[derive(Debug)]
// pub struct Drain<'a, T, const N: usize>
// where
//     T: BorshSerialize + BorshDeserialize,
// {
//     /// Mutable reference to vector used to iterate through.
//     vec: &'a mut ChunkedVector<T, N>,
//     /// Range of indices to iterate.
//     range: Range<u32>,
//     /// Range of elements to delete.
//     delete_range: Range<u32>,
// }

// impl<'a, T, const N: usize> Drain<'a, T, N>
// where
//     T: BorshSerialize + BorshDeserialize,
// {
//     /// Creates a new iterator for the given storage vector.
//     pub(crate) fn new(vec: &'a mut ChunkedVector<T, N>, range: Range<u32>) -> Self {
//         Self {
//             vec,
//             delete_range: range.clone(),
//             range,
//         }
//     }

//     /// Returns the amount of remaining elements to yield by the iterator.
//     fn remaining(&self) -> usize {
//         self.range.len()
//     }
//     fn remove(&mut self, index: u32) -> T {
//         // TODO this is unsafe and should be fixed when underlying array is MaybeUninit
//         let zeroed = unsafe { MaybeUninit::<T>::zeroed().assume_init() };
//         core::mem::replace(
//             super::expect_consistent_state(self.vec.get_mut(index)),
//             zeroed,
//         )
//     }
// }

// impl<'a, T, const N: usize> Drop for Drain<'a, T, N>
// where
//     T: BorshSerialize + BorshDeserialize,
// {
//     fn drop(&mut self) {
//         // TODO this is broken for sure
//         let delete_indices = (self.delete_range.start..self.range.start)
//             .chain(self.range.end..self.delete_range.end);

//         // Delete any non-deleted elements from iterator (not loading from storage)
//         for i in delete_indices {
//             self.vec.values.set(i, None);
//         }

//         // Shift values after delete into slots deleted.
//         let shift_len = self.delete_range.len() as u32;
//         for i in self.delete_range.end..self.vec.len() {
//             self.vec.swap(i, i - shift_len);
//         }

//         // Adjust length of vector.
//         self.vec.len -= self.delete_range.len() as u32;
//     }
// }

// impl<'a, T, const N: usize> Iterator for Drain<'a, T, N>
// where
//     T: BorshSerialize + BorshDeserialize,
// {
//     type Item = T;

//     fn next(&mut self) -> Option<Self::Item> {
//         // Load and replace value at next index
//         let delete_idx = self.range.next()?;
//         let prev = self.remove(delete_idx);

//         Some(prev)
//     }

//     fn nth(&mut self, n: usize) -> Option<Self::Item> {
//         for _ in 0..n {
//             let next = self.range.next()?;
//             // Delete all values in advance, values will be shifted over on drop.
//             // This avoids having to load and deserialize any elements skipped over.
//             self.vec.values.set(next, None);
//         }
//         self.next()
//     }

//     fn size_hint(&self) -> (usize, Option<usize>) {
//         let remaining = self.remaining();
//         (remaining, Some(remaining))
//     }

//     fn count(self) -> usize {
//         self.remaining()
//     }
// }

// impl<'a, T, const N: usize> ExactSizeIterator for Drain<'a, T, N> where
//     T: BorshSerialize + BorshDeserialize
// {
// }
// impl<'a, T, const N: usize> FusedIterator for Drain<'a, T, N> where
//     T: BorshSerialize + BorshDeserialize
// {
// }

// impl<'a, T, const N: usize> DoubleEndedIterator for Drain<'a, T, N>
// where
//     T: BorshSerialize + BorshDeserialize,
// {
//     fn next_back(&mut self) -> Option<Self::Item> {
//         let delete_idx = self.range.next_back()?;
//         let prev = self.remove(delete_idx);

//         Some(prev)
//     }

//     fn nth_back(&mut self, n: usize) -> Option<Self::Item> {
//         // Only delete and don't load any values before n
//         for _ in 0..n {
//             let next = self.range.next_back()?;
//             // Delete all values in advance, values will be shifted over on drop.
//             // This avoids having to load and deserialize any elements skipped over.
//             self.vec.values.set(next, None);
//         }
//         self.next_back()
//     }
// }
