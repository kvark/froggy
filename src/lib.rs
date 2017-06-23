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
#![doc(html_root_url = "https://docs.rs/froggy/0.3.0")]

extern crate spin;

mod bitfield;
mod cursor;
mod weak;

use spin::Mutex;
use std::iter::FromIterator;
use std::marker::PhantomData;
use std::{ops, slice};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering, ATOMIC_USIZE_INIT};
use std::vec::Drain;
use bitfield::PointerData;

pub use cursor::{CursorItem, Slice};
pub use weak::WeakPointer;

type Index = usize;

/// Reference counter type. It doesn't make sense to allocate too much bit for it in regular applications.
// TODO: control by a cargo feature
type RefCount = u16;

/// Epoch type determines the number of overwrites of components in storage.
/// TODO: control by a cargo feature
type Epoch = u16;

type StorageId = u8;
static STORAGE_UID: AtomicUsize = ATOMIC_USIZE_INIT;

/// The error type which is returned from upgrading
/// [`WeakPointer`](struct.WeakPointer.html).
#[derive(Debug, PartialEq)]
pub struct DeadComponentError;

/// Inner storage data that is locked by `RwLock`.
#[derive(Debug)]
struct StorageInner<T> {
    data: Vec<T>,
    meta: Vec<RefCount>,
    free_list: Vec<PointerData>,
}

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
/// Component storage type.
/// Manages the components and allows for efficient processing.
/// See also: [Pointer](struct.Pointer.html)
/// # Examples
/// ```rust
/// # use froggy::Storage;
/// let mut storage = Storage::new();
/// // add component to storage
/// let pointer = storage.create(1u32);
/// // change component by pointer
/// storage[&pointer] = 30;
/// ```
#[derive(Debug)]
pub struct Storage<T> {
    inner: StorageInner<T>,
    pending: PendingRef,
    id: StorageId,
}

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
#[derive(Debug)]
pub struct Pointer<T> {
    data: PointerData,
    pending: PendingRef,
    marker: PhantomData<T>,
}

impl<T> Pointer<T> {
    /// Creates a new `WeakPointer` to this component.
    /// See [`WeakPointer`](weak/struct.WeakPointer.html)
    #[inline]
    pub fn downgrade(&self) -> WeakPointer<T> {
        weak::from_pointer(self)
    }
}

/// Iterator for reading components.
#[derive(Debug)]
pub struct Iter<'a, T: 'a> {
    storage: &'a StorageInner<T>,
    skip_lost: bool,
    index: Index,
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

/// Streaming iterator providing mutable components
/// and a capability to look back/ahead.
///
/// See documentation of [`CursorItem`](struct.CursorItem.html).
#[derive(Debug)]
pub struct Cursor<'a, T: 'a> {
    storage: &'a mut StorageInner<T>,
    pending: &'a PendingRef,
    index: Index,
    storage_id: StorageId,
}


impl<'a, T> ops::Index<&'a Pointer<T>> for Storage<T> {
    type Output = T;
    #[inline]
    fn index(&self, pointer: &'a Pointer<T>) -> &T {
        debug_assert_eq!(pointer.data.get_storage_id(), self.id);
        debug_assert!(pointer.data.get_index() < self.inner.data.len());
        unsafe { self.inner.data.get_unchecked(pointer.data.get_index()) }
    }
}

impl<'a, T> ops::IndexMut<&'a Pointer<T>> for Storage<T> {
    #[inline]
    fn index_mut(&mut self, pointer: &'a Pointer<T>) -> &mut T {
        debug_assert_eq!(pointer.data.get_storage_id(), self.id);
        debug_assert!(pointer.data.get_index() < self.inner.data.len());
        unsafe { self.inner.data.get_unchecked_mut(pointer.data.get_index()) }
    }
}

impl<T> FromIterator<T> for Storage<T> {
    fn from_iter<I>(iter: I) -> Self where I: IntoIterator<Item=T> {
        let data: Vec<T> = iter.into_iter().collect();
        let count = data.len();
        Storage::new_impl(data, vec![0; count], vec![0; count])
    }
}

impl<'a, T> IntoIterator for &'a Storage<T> {
    type Item = Item<'a, T>;
    type IntoIter = Iter<'a, T>;
    fn into_iter(self) -> Self::IntoIter {
        Iter {
            storage: &self.inner,
            skip_lost: true,
            index: 0,
        }
    }
}

impl<'a, T> IntoIterator for &'a mut Storage<T> {
    type Item = &'a mut T;
    type IntoIter = IterMut<'a, T>;
    fn into_iter(self) -> Self::IntoIter {
        self.iter_mut()
    }
}

impl<T> Storage<T> {
    fn new_impl(data: Vec<T>, meta: Vec<RefCount>, epoch: Vec<Epoch>) -> Storage<T> {
        assert_eq!(data.len(), meta.len());
        assert!(epoch.len() <= meta.len());
        let uid = STORAGE_UID.fetch_add(1, Ordering::Relaxed) as StorageId;
        Storage {
            inner: StorageInner {
                data: data,
                meta: meta,
                free_list: Vec::new(),
            },
            pending: Arc::new(Mutex::new(Pending {
                add_ref: Vec::new(),
                sub_ref: Vec::new(),
                epoch: epoch,
            })),
            id: uid,
        }
    }

    /// Create a new empty storage.
    pub fn new() -> Storage<T> {
        Self::new_impl(Vec::new(), Vec::new(), Vec::new())
    }

    /// Create a new empty storage with specified capacity.
    pub fn with_capacity(capacity: usize) -> Storage<T> {
        Self::new_impl(
            Vec::with_capacity(capacity),
            Vec::with_capacity(capacity),
            Vec::with_capacity(capacity))
    }

    /// Synchronize for all the pending updates.
    /// It will update all reference counters in Storage, so
    /// [`iter_alive`](struct.Storage.html#method.iter_alive) and
    /// [`iter_alive_mut`](struct.Storage.html#method.iter_alive_mut) will return actual information.
    ///
    /// Use this function only if necessary, because it needs to block Storage.
    pub fn sync_pending(&mut self)
    {
        let mut pending = self.pending.lock();
        // missing epochs
        while pending.epoch.len() < self.inner.data.len() {
            pending.epoch.push(0);
        }
        // pending reference adds
        for index in pending.add_ref.drain(..) {
            self.inner.meta[index] += 1;
        }
        // pending reference subs
        {
            let (refs, mut epoch) = pending.drain_sub();
            for index in refs {
                self.inner.meta[index] -= 1;
                if self.inner.meta[index] == 0 {
                    epoch[index] += 1;
                    let data = PointerData::new(index, epoch[index], self.id);
                    self.inner.free_list.push(data);
                }
            }
        }
    }

    /// Iterate all components in this storage that are still referenced from outside.
    /// ### Attention
    /// Information about live components is updated not for all changes, but
    /// only when you explicitly call [`sync_pending`](struct.Storage.html#method.sync_pending).
    /// It means, you can get wrong results when calling this function before updating pending.
    #[inline]
    pub fn iter(&self) -> Iter<T> {
        Iter {
            storage: &self.inner,
            skip_lost: true,
            index: 0,
        }
    }

    /// Iterate all components that are stored, even if not referenced.
    /// This can be faster than the regular `iter` for the lack of refcount checks.
    #[inline]
    pub fn iter_all(&self) -> Iter<T> {
        Iter {
            storage: &self.inner,
            skip_lost: false,
            index: 0,
        }
    }

    /// Iterate all components in this storage that are still referenced from outside, mutably.
    /// ### Attention
    /// Information about live components is updated not for all changes, but
    /// only when you explicitly call [`sync_pending`](struct.Storage.html#method.sync_pending).
    /// It means, you can get wrong results when calling this function before updating pending.
    #[inline]
    pub fn iter_mut(&mut self) -> IterMut<T> {
        IterMut {
            data: self.inner.data.iter_mut(),
            meta: self.inner.meta.iter(),
        }
    }

    /// Iterate all components that are stored, even if not referenced, mutably.
    /// This can be faster than the regular `iter_mut` for the lack of refcount checks.
    #[inline]
    pub fn iter_all_mut(&mut self) -> slice::IterMut<T> {
        self.inner.data.iter_mut()
    }

    /// Pin an iterated item with a newly created `Pointer`.
    pub fn pin(&self, item: &Item<T>) -> Pointer<T> {
        let mut pending = self.pending.lock();
        pending.add_ref.push(item.index);
        Pointer {
            data: PointerData::new(
                item.index,
                pending.get_epoch(item.index),
                self.id,
            ),
            pending: self.pending.clone(),
            marker: PhantomData,
        }
    }

    /// Produce a streaming mutable iterator over components that are still referenced.
    /// ### Attention
    /// Information about live components is updated not for all changes, but
    /// only when you explicitly call [`sync_pending`](struct.Storage.html#method.sync_pending).
    /// It means, you can get wrong results when calling this function before updating pending.
    #[inline]
    pub fn cursor(&mut self) -> Cursor<T> {
        Cursor {
            storage: &mut self.inner,
            pending: &self.pending,
            index: 0,
            storage_id: self.id,
        }
    }

    /// Add a new component to the storage, returning the `Pointer` to it.
    pub fn create(&mut self, value: T) -> Pointer<T> {
        let data = match self.inner.free_list.pop() {
            Some(data) => {
                let i = data.get_index();
                debug_assert_eq!(self.inner.meta[i], 0);
                self.inner.data[i] = value;
                self.inner.meta[i] = 1;
                data
            },
            None => {
                let i = self.inner.meta.len();
                debug_assert_eq!(self.inner.data.len(), i);
                self.inner.data.push(value);
                self.inner.meta.push(1);
                PointerData::new(i, 0, self.id)
            },
        };
        Pointer {
            data: data,
            pending: self.pending.clone(),
            marker: PhantomData,
        }
    }
}

impl<T> Default for Storage<T> {
    fn default() -> Self {
        Self::new()
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

impl<'a, T> Iterator for Iter<'a, T> {
    type Item = Item<'a, T>;
    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let id = self.index;
            if id >= self.storage.data.len() {
                return None
            }
            self.index += 1;
            if !self.skip_lost || unsafe {*self.storage.meta.get_unchecked(id)} != 0 {
                return Some(Item {
                    value: unsafe { self.storage.data.get_unchecked(id) },
                    index: id,
                })
            }
        }
    }
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
