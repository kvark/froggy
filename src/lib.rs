/*!
Component Graph System prototype.

Froggy is all about the smart component storage, unambiguously called `Storage`.
Components inside it are automatically reference-counted, and could be referenced by a `Pointer`.
The components are stored linearly, allowing for the efficient bulk data processing.
`Storage` has to be locked temporarily for either read or write before any usage.

You can find more information about Component Graph System concept on the [wiki](https://github.com/kvark/froggy/wiki/Component-Graph-System).
Comparing to Entity-Component Systems (ECS), CGS doesn't have the backwards relation of components to entities.
Thus, it can't process all "entities" by just selecting a subset of components to work on, besides not having the whole "entity" concept.
However, CGS has a number of advantages:

  - you can share components naturally
  - you don't need to care about the component lifetime, it is managed automatically
  - you can have deeper hierarchies of components, with one component referencing the others
  - you can have user structures referencing components freely
  - there are no restrictions on the component types, and no need to implement any traits

*/
#![warn(missing_docs)]
#![doc(html_root_url = "https://docs.rs/froggy/0.4.4")]

use spin::Mutex;
use std::{
    fmt,
    hash::{Hash, Hasher},
    marker::PhantomData,
    ops, slice,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    vec::Drain,
};

mod bitfield;
mod cursor;
mod storage;
mod weak;

use crate::bitfield::PointerData;
use crate::storage::StorageInner;

pub use crate::cursor::{Cursor, CursorItem, Slice};
pub use crate::storage::Storage;
pub use crate::weak::WeakPointer;

type Index = usize;

/// Reference counter type. It doesn't make sense to allocate too much bit for it in regular applications.
// TODO: control by a cargo feature
type RefCount = u16;

/// Epoch type determines the number of overwrites of components in storage.
// TODO: control by a cargo feature
type Epoch = u16;

type StorageId = u8;
static STORAGE_UID: AtomicUsize = AtomicUsize::new(0);

/// The error type which is returned from upgrading
/// [`WeakPointer`](struct.WeakPointer.html).
#[derive(Debug, PartialEq)]
pub struct DeadComponentError;

/// Pending reference counts updates.
#[derive(Debug)]
struct Pending {
    add_ref: Vec<Index>,
    sub_ref: Vec<Index>,
    epoch: Vec<Epoch>,
}

impl Pending {
    #[inline]
    fn drain_sub(&mut self) -> (Drain<Index>, &mut [Epoch]) {
        (self.sub_ref.drain(..), self.epoch.as_mut_slice())
    }

    #[inline]
    fn get_epoch(&self, index: usize) -> Epoch {
        *self.epoch.get(index).unwrap_or(&0)
    }
}

/// Shared pointer to the pending updates.
type PendingRef = Arc<Mutex<Pending>>;

/// A pointer to a component of type `T`.
/// The component is guaranteed to be accessible for as long as this pointer is alive.
/// You'd need a storage to access the data.
/// # Examples
/// ```rust
/// # let mut storage = froggy::Storage::new();
/// // you can create pointer by creating component in storage
/// let ptr1 = storage.create(1i32);
/// // later you can change component in storage
/// storage[&ptr1] = 2i32;
/// ```
/// Also you can use [`Storage::pin`](struct.Storage.html#method.pin) to pin component with `Pointer`
///
/// ```rust
/// # let mut storage = froggy::Storage::new();
/// # let ptr1 = storage.create(1i32);
/// let item = storage.iter().next().unwrap();
/// let ptr2 = storage.pin(&item);
/// // Pointers to the same component are equal
/// assert_eq!(ptr1, ptr2);
/// ```
pub struct Pointer<T> {
    data: PointerData,
    pending: PendingRef,
    marker: PhantomData<T>,
}

impl<T> fmt::Debug for Pointer<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        /// Debug output type for `Self`.
        #[derive(Debug)]
        pub struct Pointer<'a> {
            /// All integer entries are `usize` for future-proofing.
            index: usize,
            epoch: usize,
            storage_id: usize,
            pending: &'a Pending,
        }

        fmt::Debug::fmt(
            &Pointer {
                index: self.data.get_index() as usize,
                epoch: self.data.get_epoch() as usize,
                storage_id: self.data.get_storage_id() as usize,
                pending: &self.pending.lock(),
            },
            f,
        )
    }
}

impl<T> Pointer<T> {
    /// Creates a new `WeakPointer` to this component.
    /// See [`WeakPointer`](weak/struct.WeakPointer.html)
    #[inline]
    pub fn downgrade(&self) -> WeakPointer<T> {
        weak::from_pointer(self)
    }
}

impl<T> PartialOrd for Pointer<T> {
    fn partial_cmp(&self, other: &Pointer<T>) -> Option<std::cmp::Ordering> {
        if self.data.get_storage_id() == other.data.get_storage_id() {
            debug_assert!(
                self.data.get_index() != other.data.get_index()
                    || self.data.get_epoch() == self.data.get_epoch()
            );
            self.data.get_index().partial_cmp(&other.data.get_index())
        } else {
            None
        }
    }
}

impl<T> Clone for Pointer<T> {
    #[inline]
    fn clone(&self) -> Pointer<T> {
        self.pending.lock().add_ref.push(self.data.get_index());
        Pointer {
            data: self.data,
            pending: self.pending.clone(),
            marker: PhantomData,
        }
    }
}

impl<T> PartialEq for Pointer<T> {
    #[inline]
    fn eq(&self, other: &Pointer<T>) -> bool {
        self.data == other.data
    }
}

impl<T> Eq for Pointer<T> {}

impl<T> Hash for Pointer<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.data.hash(state);
    }
}

impl<T> Drop for Pointer<T> {
    #[inline]
    fn drop(&mut self) {
        self.pending.lock().sub_ref.push(self.data.get_index());
    }
}

/// The item of `Iter`.
#[derive(Debug, Clone, Copy)]
pub struct Item<'a, T: 'a> {
    value: &'a T,
    index: Index,
}

impl<'a, T> ops::Deref for Item<'a, T> {
    type Target = T;
    fn deref(&self) -> &T {
        self.value
    }
}

/// Iterator for reading components.
#[derive(Debug)]
pub struct Iter<'a, T: 'a> {
    storage: &'a StorageInner<T>,
    skip_lost: bool,
    index: Index,
}

impl<'a, T> Iterator for Iter<'a, T> {
    type Item = Item<'a, T>;
    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let id = self.index;
            if id >= self.storage.data.len() {
                return None;
            }
            self.index += 1;
            if !self.skip_lost || unsafe { *self.storage.meta.get_unchecked(id) } != 0 {
                return Some(Item {
                    value: unsafe { self.storage.data.get_unchecked(id) },
                    index: id,
                });
            }
        }
    }
}

impl<'a, T> Clone for Iter<'a, T> {
    fn clone(&self) -> Self {
        Iter {
            storage: self.storage,
            skip_lost: self.skip_lost,
            index: self.index,
        }
    }
}

/// Iterator for writing components.
#[derive(Debug)]
pub struct IterMut<'a, T: 'a> {
    data: slice::IterMut<'a, T>,
    meta: slice::Iter<'a, RefCount>,
}

impl<'a, T> Iterator for IterMut<'a, T> {
    type Item = &'a mut T;
    fn next(&mut self) -> Option<Self::Item> {
        while let Some(&0) = self.meta.next() {
            self.data.next();
        }
        self.data.next()
    }
}

impl<'a, T> DoubleEndedIterator for IterMut<'a, T> {
    fn next_back(&mut self) -> Option<Self::Item> {
        while let Some(&0) = self.meta.next_back() {
            self.data.next_back();
        }
        self.data.next_back()
    }
}
