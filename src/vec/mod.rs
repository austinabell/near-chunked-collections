//! A growable array type with values persisted to storage in chunks and lazily loaded.
//!
//! Values in the [`Vector`] are kept in an in-memory cache and are only persisted on [`Drop`].
//!
//! Vectors ensure they never allocate more than [`u32::MAX`] bytes. [`u32`] is used rather than
//! [`usize`] as in [`Vec`] to ensure consistent behavior on different targets.
//!
//! # Examples
//!
//! You can explicitly create a [`Vector`] with [`Vector::new`]:
//!
//! ```
//! use near_sdk::store::Vector;
//!
//! let v: Vector<i32> = Vector::new(b"a");
//! ```
//!
//! You can [`push`](Vector::push) values onto the end of a vector (which will grow the vector
//! as needed):
//!
//! ```
//! use near_sdk::store::Vector;
//!
//! let mut v: Vector<i32> = Vector::new(b"a");
//!
//! v.push(3);
//! ```
//!
//! Popping values works in much the same way:
//!
//! ```
//! use near_sdk::store::Vector;
//!
//! let mut v: Vector<i32> = Vector::new(b"a");
//! v.extend([1, 2]);
//!
//! let two = v.pop();
//! ```
//!
//! Vectors also support indexing (through the [`Index`] and [`IndexMut`] traits):
//!
//! ```
//! use near_sdk::store::Vector;
//!
//! let mut v: Vector<i32> = Vector::new(b"a");
//! v.extend([1, 2, 3]);
//!
//! let three = v[2];
//! v[1] = v[1] + 5;
//! ```
//!
//! [`Index`]: std::ops::Index
//! [`IndexMut`]: std::ops::IndexMut

mod impls;
mod iter;

use core::mem::MaybeUninit;
use std::fmt;

use borsh::{BorshDeserialize, BorshSerialize};

// pub use self::iter::{Drain, Iter, IterMut};
pub use self::iter::{Iter, IterMut};
use near_sdk::{env, IntoStorageKey};

use near_sdk::store::index_map::IndexMap;

const ERR_INDEX_OUT_OF_BOUNDS: &str = "Index out of bounds";

fn expect_consistent_state<T>(val: Option<T>) -> T {
    val.unwrap_or_else(|| env::panic_str("inconsistent state"))
}

fn chunk_index<const N: usize>(index: u32) -> u32 {
    // TODO yeah this is a bit unsafe if N is > 32 bits range. Fix
    (index as usize / N) as u32
}

fn chunk_pos<const N: usize>(index: u32) -> usize {
    index as usize % N
}

/// An iterable implementation of vector that stores its content on the trie. This implementation
/// will load and store values in the underlying storage lazily.
///
/// Uses the following map: index -> element. Because the data is sharded to avoid reading/writing
/// large chunks of data, the values cannot be accessed as a contiguous piece of memory.
///
/// This implementation will cache all changes and loads and only updates values that are changed
/// in storage after it's dropped through it's [`Drop`] implementation. These changes can be updated
/// in storage before the variable is dropped by using [`Vector::flush`]. During the lifetime of
/// this type, storage will only be read a maximum of one time per index and only written once per
/// index unless specifically flushed.
///
/// This type should be a drop in replacement for [`Vec`] in most cases and will provide contracts
/// a vector structure which scales much better as the contract data grows.
///
/// # Examples
/// ```
/// use near_sdk::store::Vector;
///
/// let mut vec = Vector::new(b"a");
/// assert!(vec.is_empty());
///
/// vec.push(1);
/// vec.push(2);
///
/// assert_eq!(vec.len(), 2);
/// assert_eq!(vec[0], 1);
///
/// assert_eq!(vec.pop(), Some(2));
/// assert_eq!(vec.len(), 1);
///
/// vec[0] = 7;
/// assert_eq!(vec[0], 7);
///
/// vec.extend([1, 2, 3].iter().copied());
/// assert!(Iterator::eq(vec.into_iter(), [7, 1, 2, 3].iter()));
/// ```
// TODO decide on a default chunk size
pub struct ChunkedVector<T, const N: usize = 5>
where
    T: BorshSerialize,
{
    pub(crate) len: u32,
    // TODO this can theoretically be IndexMap<[MaybeUninit<T>; N]> to avoid using Default
    pub(crate) values: IndexMap<[T; N]>,
}

impl<T, const N: usize> Drop for ChunkedVector<T, N>
where
    T: BorshSerialize,
{
    fn drop(&mut self) {
        self.flush()
    }
}

//? Manual implementations needed only because borsh derive is leaking field types
// https://github.com/near/borsh-rs/issues/41
impl<T, const N: usize> BorshSerialize for ChunkedVector<T, N>
where
    T: BorshSerialize,
{
    fn serialize<W: borsh::maybestd::io::Write>(
        &self,
        writer: &mut W,
    ) -> Result<(), borsh::maybestd::io::Error> {
        BorshSerialize::serialize(&self.len, writer)?;
        BorshSerialize::serialize(&self.values, writer)?;
        Ok(())
    }
}

impl<T, const N: usize> BorshDeserialize for ChunkedVector<T, N>
where
    T: BorshSerialize,
{
    fn deserialize(buf: &mut &[u8]) -> Result<Self, borsh::maybestd::io::Error> {
        Ok(Self {
            len: BorshDeserialize::deserialize(buf)?,
            values: BorshDeserialize::deserialize(buf)?,
        })
    }
}

impl<T, const N: usize> ChunkedVector<T, N>
where
    T: BorshSerialize,
{
    /// Returns the number of elements in the vector, also referred to as its size.
    /// This function returns a `u32` rather than the [`Vec`] equivalent of `usize` to have
    /// consistency between targets.
    ///
    /// # Examples
    ///
    /// ```
    /// use near_sdk::store::Vector;
    ///
    /// let mut vec = Vector::new(b"a");
    /// vec.push(1);
    /// vec.push(2);
    /// assert_eq!(vec.len(), 2);
    /// ```
    pub fn len(&self) -> u32 {
        self.len
    }

    /// Returns `true` if the vector contains no elements.
    ///
    /// # Examples
    ///
    /// ```
    /// use near_sdk::store::Vector;
    ///
    /// let mut vec = Vector::new(b"a");
    /// assert!(vec.is_empty());
    ///
    /// vec.push(1);
    /// assert!(!vec.is_empty());
    /// ```
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Create new vector with zero elements. Prefixes storage accesss with the prefix provided.
    ///
    /// This prefix can be anything that implements [`IntoStorageKey`]. The prefix is used when
    /// storing and looking up values in storage to ensure no collisions with other collections.
    ///
    /// # Examples
    ///
    /// ```
    /// use near_sdk::store::Vector;
    ///
    /// let mut vec: Vector<u8> = Vector::new(b"a");
    /// ```
    pub fn new<S>(prefix: S) -> Self
    where
        S: IntoStorageKey,
    {
        Self {
            len: 0,
            values: IndexMap::new(prefix),
        }
    }

    /// Removes all elements from the collection. This will remove all storage values for the
    /// length of the [`Vector`].
    ///
    /// # Examples
    ///
    /// ```
    /// use near_sdk::store::Vector;
    ///
    /// let mut vec = Vector::new(b"a");
    /// vec.push(1);
    ///
    /// vec.clear();
    ///
    /// assert!(vec.is_empty());
    /// ```
    pub fn clear(&mut self) {
        for i in 0..self.len {
            self.values.set(i, None);
        }
        self.len = 0;
    }

    /// Flushes the cache and writes all modified values to storage.
    ///
    /// This operation is performed on [`Drop`], but this method can be called to persist
    /// intermediate writes in cases where [`Drop`] is not called or to identify storage changes.
    pub fn flush(&mut self) {
        self.values.flush();
    }
}

impl<T, const N: usize> ChunkedVector<T, N>
where
    T: BorshSerialize + BorshDeserialize,
{
    /// Appends an element to the back of the collection.
    ///
    /// # Panics
    ///
    /// Panics if new length exceeds `u32::MAX`
    ///
    /// # Examples
    ///
    /// ```
    /// use near_sdk::store::Vector;
    ///
    /// let mut vec = Vector::new(b"v");
    /// vec.push("test".to_string());
    ///
    /// assert!(!vec.is_empty());
    /// ```
    pub fn push(&mut self, element: T) {
        let last_idx = self.len();
        self.len = self
            .len
            .checked_add(1)
            .unwrap_or_else(|| env::panic_str(ERR_INDEX_OUT_OF_BOUNDS));

        let chunk_idx = chunk_index::<N>(last_idx);
        let chunk_pos = chunk_pos::<N>(last_idx);
        if chunk_pos == 0 {
            // Push is on new chunk, create new chunk
            let chunk = MaybeUninit::<[T; N]>::zeroed();
            // TODO this is unsafe for drop impls on zeroed data. Fix for actual use
            let mut chunk = unsafe { chunk.assume_init() };
            chunk[0] = element;
            self.values.set(chunk_idx, Some(chunk));
        } else {
            // Chunk already exists, update the index in the chunk.
            // TODO would be ideal to be able to replace the data only at the index, not deserialize
            // TODO ..the whole chunk. This would require fixed serialization sizes, though.
            expect_consistent_state(self.values.get_mut(chunk_idx))[chunk_pos] = element;
        }
    }

    /// Returns the element by index or `None` if it is not present.
    ///
    /// # Examples
    ///
    /// ```
    /// use near_sdk::store::Vector;
    ///
    /// let mut vec = Vector::new(b"v");
    /// vec.push("test".to_string());
    ///
    /// assert_eq!(Some(&"test".to_string()), vec.get(0));
    /// assert_eq!(None, vec.get(3));
    /// ```
    pub fn get(&self, index: u32) -> Option<&T> {
        if index >= self.len() {
            return None;
        }

        self.values
            .get(chunk_index::<N>(index))
            .map(|chunk| &chunk[chunk_pos::<N>(index)])
    }

    /// Returns a mutable reference to the element at the `index` provided.
    ///
    /// # Examples
    ///
    /// ```
    /// use near_sdk::store::Vector;
    ///
    /// let mut vec = Vector::new(b"v");
    /// let x = vec![0, 1, 2];
    /// vec.extend(x);
    ///
    /// if let Some(elem) = vec.get_mut(1) {
    ///     *elem = 42;
    /// }
    ///
    /// let actual: Vec<_> = vec.iter().cloned().collect();
    /// assert_eq!(actual, &[0, 42, 2]);
    /// ```
    pub fn get_mut(&mut self, index: u32) -> Option<&mut T> {
        if index >= self.len {
            return None;
        }

        self.values
            .get_mut(chunk_index::<N>(index))
            .map(|chunk| &mut chunk[chunk_pos::<N>(index)])
    }

    fn swap(&mut self, a: u32, b: u32) {
        if a >= self.len() || b >= self.len() {
            env::panic_str(ERR_INDEX_OUT_OF_BOUNDS);
        }

        if a == b {
            return;
        }

        let a_idx = chunk_index::<N>(a);
        if a_idx == chunk_index::<N>(b) {
            // Values are on the same chunk, swap.
            let chunk = self.values.get_mut(a_idx).unwrap();
            chunk.swap(chunk_pos::<N>(a), chunk_pos::<N>(b));
        } else {
            // Values are on different chunks, swap across chunks.
            // TODO maybe a cleaner or safer way to do this.
            let a_mut: &mut T =
                unsafe { &mut *(expect_consistent_state(self.get_mut(a)) as *mut _) };
            let b_mut = expect_consistent_state(self.get_mut(b));

            core::mem::swap(a_mut, b_mut);
        }
    }

    /// Removes an element from the vector and returns it.
    /// The removed element is replaced by the last element of the vector.
    /// Does not preserve ordering, but is `O(1)`.
    ///
    /// # Panics
    ///
    /// Panics if `index` is out of bounds.
    ///
    /// # Examples
    ///
    /// ```
    /// use near_sdk::store::Vector;
    ///
    /// let mut vec: Vector<u8> = Vector::new(b"v");
    /// vec.extend([1, 2, 3, 4]);
    ///
    /// assert_eq!(vec.swap_remove(1), 2);
    /// assert_eq!(vec.iter().copied().collect::<Vec<_>>(), &[1, 4, 3]);
    ///
    /// assert_eq!(vec.swap_remove(0), 1);
    /// assert_eq!(vec.iter().copied().collect::<Vec<_>>(), &[3, 4]);
    /// ```
    pub fn swap_remove(&mut self, index: u32) -> T {
        if self.is_empty() {
            env::panic_str(ERR_INDEX_OUT_OF_BOUNDS);
        }

        self.swap(index, self.len() - 1);
        expect_consistent_state(self.pop())
    }

    /// Removes the last element from a vector and returns it, or [`None`] if it is empty.
    ///
    /// # Examples
    ///
    /// ```
    /// use near_sdk::store::Vector;
    ///
    /// let mut vec = Vector::new(b"v");
    /// vec.extend([1, 2, 3]);
    ///
    /// assert_eq!(vec.pop(), Some(3));
    /// assert_eq!(vec.pop(), Some(2));
    /// ```
    pub fn pop(&mut self) -> Option<T> {
        let new_idx = self.len.checked_sub(1)?;
        let pop_position = chunk_pos::<N>(new_idx);
        let prev = if pop_position == 0 {
            // The element being popped is only one in chunk, remove the chunk and return the first
            // element, which is the one being popped.
            expect_consistent_state(self.values.remove(chunk_index::<N>(new_idx)))
                .into_iter()
                .next()
        } else {
            // TODO this is broken to assume init for zeroed for faulty drop impls.
            let zeroed_element = unsafe { MaybeUninit::<T>::zeroed().assume_init() };
            self.values
                .get_mut(chunk_index::<N>(new_idx))
                .map(|chunk| core::mem::replace(&mut chunk[pop_position], zeroed_element))
        };
        self.len = new_idx;
        prev
    }

    /// Returns an iterator over the vector. This iterator will lazily load any values iterated
    /// over from storage.
    ///
    /// # Examples
    ///
    /// ```
    /// use near_sdk::store::Vector;
    ///
    /// let mut vec = Vector::new(b"v");
    /// vec.extend([1, 2, 4]);
    /// let mut iterator = vec.iter();
    ///
    /// assert_eq!(iterator.next(), Some(&1));
    /// assert_eq!(iterator.next(), Some(&2));
    /// assert_eq!(iterator.next(), Some(&4));
    /// assert_eq!(iterator.next(), None);
    /// ```
    pub fn iter(&self) -> Iter<T, N> {
        Iter::new(self)
    }

    /// Returns an iterator over the [`Vector`] that allows modifying each value. This iterator
    /// will lazily load any values iterated over from storage.
    ///
    /// # Examples
    ///
    /// ```
    /// use near_sdk::store::Vector;
    ///
    /// let mut vec = Vector::new(b"v");
    /// vec.extend([1u32, 2, 4]);
    ///
    /// for elem in vec.iter_mut() {
    ///     *elem += 2;
    /// }
    /// assert_eq!(vec.iter().copied().collect::<Vec<_>>(), &[3u32, 4, 6]);
    /// ```
    pub fn iter_mut(&mut self) -> IterMut<T, N> {
        IterMut::new(self)
    }

    // /// Creates a draining iterator that removes the specified range in the vector
    // /// and yields the removed items.
    // ///
    // /// When the iterator **is** dropped, all elements in the range are removed
    // /// from the vector, even if the iterator was not fully consumed. If the
    // /// iterator **is not** dropped (with [`mem::forget`](std::mem::forget) for example),
    // /// the collection will be left in an inconsistent state.
    // ///
    // /// This will not panic on invalid ranges (`end > length` or `end < start`) and instead the
    // /// iterator will just be empty.
    // ///
    // /// # Examples
    // ///
    // /// ```
    // /// use near_sdk::store::Vector;
    // ///
    // /// let mut vec: Vector<u32> = Vector::new(b"v");
    // /// vec.extend(vec![1, 2, 3]);
    // ///
    // /// let u: Vec<_> = vec.drain(1..).collect();
    // /// assert_eq!(vec.iter().copied().collect::<Vec<_>>(), &[1]);
    // /// assert_eq!(u, &[2, 3]);
    // ///
    // /// // A full range clears the vector, like `clear()` does
    // /// vec.drain(..);
    // /// assert!(vec.is_empty());
    // /// ```
    // pub fn drain<R>(&mut self, range: R) -> Drain<T, N>
    // where
    //     R: RangeBounds<u32>,
    // {
    //     let start = match range.start_bound() {
    //         Bound::Excluded(i) => i
    //             .checked_add(1)
    //             .unwrap_or_else(|| env::panic_str(ERR_INDEX_OUT_OF_BOUNDS)),
    //         Bound::Included(i) => *i,
    //         Bound::Unbounded => 0,
    //     };
    //     let end = match range.end_bound() {
    //         Bound::Excluded(i) => *i,
    //         Bound::Included(i) => i
    //             .checked_add(1)
    //             .unwrap_or_else(|| env::panic_str(ERR_INDEX_OUT_OF_BOUNDS)),
    //         Bound::Unbounded => self.len(),
    //     };

    //     // Note: don't need to do bounds check if end < start, will just return None when iterating
    //     // This will also cap the max length at the length of the vector.
    //     Drain::new(
    //         self,
    //         Range {
    //             start,
    //             end: core::cmp::min(end, self.len()),
    //         },
    //     )
    // }
}

impl<T, const N: usize> fmt::Debug for ChunkedVector<T, N>
where
    T: BorshSerialize + BorshDeserialize + fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if cfg!(feature = "expensive-debug") {
            fmt::Debug::fmt(&self.iter().collect::<Vec<_>>(), f)
        } else {
            f.debug_struct("Vector")
                .field("len", &self.len)
                .field("prefix", &self.values.prefix)
                .finish()
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[cfg(test)]
mod tests {
    use arbitrary::{Arbitrary, Unstructured};
    use borsh::{BorshDeserialize, BorshSerialize};
    use rand::{Rng, RngCore, SeedableRng};

    use super::ChunkedVector;
    use near_sdk::{store::index_map::IndexMap, test_utils::test_env::setup_free};

    #[test]
    fn test_push_pop() {
        let mut rng = rand_xorshift::XorShiftRng::seed_from_u64(0);
        let mut vec = ChunkedVector::<_>::new(b"v");
        let mut baseline = vec![];
        for _ in 0..500 {
            let value = rng.gen::<u64>();
            vec.push(value);
            baseline.push(value);
        }
        let actual: Vec<u64> = vec.iter().cloned().collect();
        assert_eq!(actual, baseline);
        for _ in 0..501 {
            assert_eq!(baseline.pop(), vec.pop());
        }
    }

    #[test]
    pub fn test_replace() {
        let mut rng = rand_xorshift::XorShiftRng::seed_from_u64(1);
        let mut vec = ChunkedVector::<_>::new(b"v");
        let mut baseline = vec![];
        for _ in 0..500 {
            let value = rng.gen::<u64>();
            vec.push(value);
            baseline.push(value);
        }
        for _ in 0..500 {
            let index = rng.gen::<u32>() % vec.len();
            let value = rng.gen::<u64>();
            let old_value0 = vec[index];
            let old_value1 = core::mem::replace(vec.get_mut(index).unwrap(), value);
            let old_value2 = baseline[index as usize];
            assert_eq!(old_value0, old_value1);
            assert_eq!(old_value0, old_value2);
            *baseline.get_mut(index as usize).unwrap() = value;
        }
        let actual: Vec<_> = vec.iter().cloned().collect();
        assert_eq!(actual, baseline);
    }

    #[test]
    pub fn test_swap_remove() {
        let mut rng = rand_xorshift::XorShiftRng::seed_from_u64(2);
        let mut vec = ChunkedVector::<_>::new(b"v");
        let mut baseline = vec![];
        for _ in 0..500 {
            let value = rng.gen::<u64>();
            vec.push(value);
            baseline.push(value);
        }
        for _ in 0..500 {
            let index = rng.gen::<u32>() % vec.len();
            let old_value0 = vec[index];
            let old_value1 = vec.swap_remove(index);
            let old_value2 = baseline[index as usize];
            let last_index = baseline.len() - 1;
            baseline.swap(index as usize, last_index);
            baseline.pop();
            assert_eq!(old_value0, old_value1);
            assert_eq!(old_value0, old_value2);
        }
        let actual: Vec<_> = vec.iter().cloned().collect();
        assert_eq!(actual, baseline);
    }

    #[test]
    pub fn test_clear() {
        let mut rng = rand_xorshift::XorShiftRng::seed_from_u64(3);
        let mut vec = ChunkedVector::<_>::new(b"v");
        for _ in 0..100 {
            for _ in 0..(rng.gen::<u64>() % 20 + 1) {
                let value = rng.gen::<u64>();
                vec.push(value);
            }
            assert!(!vec.is_empty());
            vec.clear();
            assert!(vec.is_empty());
        }
    }

    #[test]
    pub fn test_extend() {
        let mut rng = rand_xorshift::XorShiftRng::seed_from_u64(0);
        let mut vec = ChunkedVector::<_>::new(b"v");
        let mut baseline = vec![];
        for _ in 0..100 {
            let value = rng.gen::<u64>();
            vec.push(value);
            baseline.push(value);
        }

        for _ in 0..100 {
            let mut tmp = vec![];
            for _ in 0..=(rng.gen::<u64>() % 20 + 1) {
                let value = rng.gen::<u64>();
                tmp.push(value);
            }
            baseline.extend(tmp.clone());
            vec.extend(tmp.clone());
        }
        let actual: Vec<_> = vec.iter().cloned().collect();
        assert_eq!(actual, baseline);
    }

    #[test]
    fn test_debug() {
        let mut rng = rand_xorshift::XorShiftRng::seed_from_u64(4);
        let prefix = b"v";
        let mut vec = ChunkedVector::<_>::new(prefix);
        let mut baseline = vec![];
        for _ in 0..10 {
            let value = rng.gen::<u64>();
            vec.push(value);
            baseline.push(value);
        }
        let actual: Vec<_> = vec.iter().cloned().collect();
        assert_eq!(actual, baseline);
        for _ in 0..5 {
            assert_eq!(baseline.pop(), vec.pop());
        }
        if cfg!(feature = "expensive-debug") {
            assert_eq!(format!("{vec:#?}"), format!("{baseline:#?}"));
        } else {
            assert_eq!(
                format!("{vec:?}"),
                format!("Vector {{ len: 5, prefix: {:?} }}", vec.values.prefix)
            );
        }

        // * The storage is reused in the second part of this test, need to flush
        vec.flush();

        use borsh::{BorshDeserialize, BorshSerialize};
        #[derive(Debug, BorshSerialize, BorshDeserialize)]
        struct TestType(u64);

        let deserialize_only_vec = ChunkedVector::<TestType> {
            len: vec.len(),
            values: IndexMap::new(prefix),
        };
        let baseline: Vec<_> = baseline.into_iter().map(TestType).collect();
        if cfg!(feature = "expensive-debug") {
            assert_eq!(
                format!("{deserialize_only_vec:#?}"),
                format!("{baseline:#?}")
            );
        } else {
            assert_eq!(
                format!("{deserialize_only_vec:?}"),
                format!(
                    "Vector {{ len: 5, prefix: {:?} }}",
                    deserialize_only_vec.values.prefix
                )
            );
        }
    }

    #[test]
    pub fn iterator_checks() {
        let mut vec = ChunkedVector::<_>::new(b"v");
        let mut baseline = vec![];
        for i in 0..10 {
            vec.push(i);
            baseline.push(i);
        }

        let mut vec_iter = vec.iter();
        let mut bl_iter = baseline.iter();
        assert_eq!(vec_iter.next(), bl_iter.next());
        assert_eq!(vec_iter.next_back(), bl_iter.next_back());
        assert_eq!(vec_iter.nth(3), bl_iter.nth(3));
        assert_eq!(vec_iter.nth_back(2), bl_iter.nth_back(2));

        // Check to make sure indexing overflow is handled correctly
        assert!(vec_iter.nth(5).is_none());
        assert!(bl_iter.nth(5).is_none());

        assert!(vec_iter.next().is_none());
        assert!(bl_iter.next().is_none());

        // Count check
        assert_eq!(vec.iter().count(), baseline.len());
    }

    // #[test]
    // fn drain_iterator() {
    //     let mut vec = ChunkedVector::<_>::new(b"v");
    //     let mut baseline = vec![0u8, 1, 2, 3, 4, 5, 6, 7, 8, 9];
    //     vec.extend(baseline.clone());

    //     assert!(Iterator::eq(vec.drain(1..=3), baseline.drain(1..=3)));
    //     assert_eq!(
    //         vec.iter().copied().collect::<Vec<_>>(),
    //         vec![0, 4, 5, 6, 7, 8, 9]
    //     );

    //     // Test incomplete drain
    //     {
    //         let mut drain = vec.drain(0..3);
    //         let mut b_drain = baseline.drain(0..3);
    //         assert_eq!(drain.next(), b_drain.next());
    //         assert_eq!(drain.next(), b_drain.next());
    //     }

    //     // 7 elements, drained 3
    //     assert_eq!(vec.len(), 4);

    //     // Test incomplete drain over limit
    //     {
    //         let mut drain = vec.drain(2..);
    //         let mut b_drain = baseline.drain(2..);
    //         assert_eq!(drain.next(), b_drain.next());
    //     }

    //     // Drain rest
    //     assert!(Iterator::eq(vec.drain(..), baseline.drain(..)));

    //     // Test double ended iterator functions
    //     let mut vec = ChunkedVector::<_>::new(b"v");
    //     let mut baseline = vec![0u8, 1, 2, 3, 4, 5, 6, 7, 8, 9];
    //     vec.extend(baseline.clone());

    //     {
    //         let mut drain = vec.drain(1..8);
    //         let mut b_drain = baseline.drain(1..8);
    //         assert_eq!(drain.nth(1), b_drain.nth(1));
    //         assert_eq!(drain.nth_back(2), b_drain.nth_back(2));
    //         assert_eq!(drain.len(), b_drain.len());
    //     }

    //     assert_eq!(vec.len() as usize, baseline.len());
    //     assert!(Iterator::eq(vec.iter(), baseline.iter()));

    //     assert!(Iterator::eq(vec.drain(..), baseline.drain(..)));
    //     near_sdk::mock::with_mocked_blockchain(|m| assert!(m.take_storage().is_empty()));
    // }

    #[derive(Arbitrary, Debug)]
    enum Op {
        Push(u8),
        Pop,
        Remove(u32),
        Flush,
        Reset,
        Get(u32),
        Swap(u32, u32),
    }

    #[test]
    fn arbitrary() {
        setup_free();

        let mut rng = rand_xorshift::XorShiftRng::seed_from_u64(0);
        let mut buf = vec![0; 4096];
        for _ in 0..1024 {
            // Clear storage in-between runs
            near_sdk::mock::with_mocked_blockchain(|b| b.take_storage());
            rng.fill_bytes(&mut buf);

            let mut sv = ChunkedVector::<_>::new(b"v");
            let mut mv = Vec::new();
            let u = Unstructured::new(&buf);
            if let Ok(ops) = Vec::<Op>::arbitrary_take_rest(u) {
                for op in ops {
                    match op {
                        Op::Push(v) => {
                            sv.push(v);
                            mv.push(v);
                            assert_eq!(sv.len() as usize, mv.len());
                        }
                        Op::Pop => {
                            assert_eq!(sv.pop(), mv.pop());
                            assert_eq!(sv.len() as usize, mv.len());
                        }
                        Op::Remove(i) => {
                            if sv.is_empty() {
                                continue;
                            }
                            let i = i % sv.len();
                            let r1 = sv.swap_remove(i);
                            let r2 = mv.swap_remove(i as usize);
                            assert_eq!(r1, r2);
                            assert_eq!(sv.len() as usize, mv.len());
                        }
                        Op::Flush => {
                            sv.flush();
                        }
                        Op::Reset => {
                            let serialized = sv.try_to_vec().unwrap();
                            sv = ChunkedVector::deserialize(&mut serialized.as_slice()).unwrap();
                        }
                        Op::Get(k) => {
                            let r1 = sv.get(k);
                            let r2 = mv.get(k as usize);
                            assert_eq!(r1, r2)
                        }
                        Op::Swap(i1, i2) => {
                            if sv.is_empty() {
                                continue;
                            }
                            let i1 = i1 % sv.len();
                            let i2 = i2 % sv.len();
                            sv.swap(i1, i2);
                            mv.swap(i1 as usize, i2 as usize)
                        }
                    }
                }
            }

            // After all operations, compare both vectors
            assert!(Iterator::eq(sv.iter(), mv.iter()));
        }
    }

    #[test]
    fn serialized_bytes() {
        use borsh::{BorshDeserialize, BorshSerialize};

        let mut vec = ChunkedVector::<_>::new(b"v");
        vec.push("Some data".to_string());
        let serialized = vec.try_to_vec().unwrap();

        // Expected to serialize len then prefix
        let mut expected_buf = Vec::new();
        1u32.serialize(&mut expected_buf).unwrap();
        (b"v"[..]).serialize(&mut expected_buf).unwrap();

        assert_eq!(serialized, expected_buf);
        drop(vec);
        let vec = ChunkedVector::<String>::deserialize(&mut serialized.as_slice()).unwrap();
        assert_eq!(vec[0], "Some data");
    }
}
