/*!
Component Graph System prototype.

Froggy is all about the smart component storage, unambiguously called `Storage`.
Components inside it are automatically reference-counted, and could be referenced by a `Pointer`.
The components are stored linearly, allowing for the efficient bulk data processing.
`Storage` has to be locked temporarily for either read or write before any usage.

You can find more information about Component Graph System concept on the [wiki](https://github.com/kvark/froggy/wiki/Component-Graph-System).
Comparing to Entity-Component Systems (ECS), CGS doesn't have the backwards relation of components to entities.
Thus, it can't process all "entities" by just selecting a subsect of compoments to work on, besides not having the whole "entity" concept.
However, CGS has a number of advantages:

  - you can share components naturally
  - you don't need to care about the component lifetime, it is managed automatically
  - you can have deeper hierarchies of components, with one component referencing the others
  - you can have user structures referencing components freely
  - there are no restrictions on the component types, and no need to implement any traits

*/
#![warn(missing_docs)]

mod bitfield;

use std::iter::FromIterator;
use std::marker::PhantomData;
use std::ops;
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicUsize, Ordering, ATOMIC_USIZE_INIT};
use std::vec::Drain;
use bitfield::PointerData;

type Index = usize;

/// Reference counter type. It doesn't make sense to allocate too much bit for it in regular applications.
// TODO: control by a cargo feature
type RefCount = u16;

/// Epoch type determines the number of overwrites of components in storage.
/// TODO: control by a cargo feature
type Epoch = u16;

type StorageId = u8;
static STORAGE_UID: AtomicUsize = ATOMIC_USIZE_INIT;

/// The error type which is returned from upgrading WeakPointer.
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
pub struct Storage<T> {
    inner: StorageInner<T>,
    pending: PendingRef,
    id: StorageId,
}

/// A pointer to a component of type `T`.
/// The component is guaranteed to be accessible for as long as this pointer is alive.
/// You'd need a locked storage to access the data.
/// The pointer also holds the storage alive and knows the index of the element to look up.
#[derive(Debug)]
pub struct Pointer<T> {
    data: PointerData,
    pending: PendingRef,
    marker: PhantomData<T>,
}

impl<T> Pointer<T> {
    /// Creates a new `WeakPointer` to this component.
    #[inline]
    pub fn downgrade(&self) -> WeakPointer<T> {
        WeakPointer {
            data: self.data,
            pending: self.pending.clone(),
            marker: PhantomData,
        }
    }
}

/// Weak variant of `Pointer`.
pub struct WeakPointer<T> {
    data: PointerData,
    pending: PendingRef,
    marker: PhantomData<T>,
}

impl<T> WeakPointer<T> {
    /// Upgrades the `WeakPointer` to a `Pointer`, if possible.
    /// Returns `Err` if the strong count has reached zero or the inner value was destroyed.
    pub fn upgrade(&self) -> Result<Pointer<T>, DeadComponentError> {
        let mut pending = self.pending.lock().unwrap();
        if pending.get_epoch(self.data.get_index()) != self.data.get_epoch() {
            return Err(DeadComponentError);
        }
        pending.add_ref.push(self.data.get_index());
        Ok(Pointer {
            data: self.data,
            pending: self.pending.clone(),
            marker: PhantomData,
        })
    }
}

/// Iterator for reading components.
pub struct ReadIter<'a, T: 'a> {
    storage: &'a StorageInner<T>,
    skip_lost: bool,
    index: Index,
}

/// Iterator for writing components.
pub struct WriteIter<'a, T: 'a> {
    storage: &'a mut StorageInner<T>,
    skip_lost: bool,
    index: Index,
}


impl<'a, T> ops::Index<&'a Pointer<T>> for Storage<T> {
    type Output = T;
    #[inline]
    fn index(&self, pointer: &'a Pointer<T>) -> &T {
        debug_assert_eq!(self.id, pointer.data.get_storage_id());
        debug_assert!(pointer.data.get_index() < self.inner.data.len());
        unsafe { self.inner.data.get_unchecked(pointer.data.get_index()) }
    }
}

impl<'a, T> ops::IndexMut<&'a Pointer<T>> for Storage<T> {
    #[inline]
    fn index_mut(&mut self, pointer: &'a Pointer<T>) -> &mut T {
        debug_assert_eq!(self.id, pointer.storage_id);
        debug_assert!(pointer.index < self.inner.data.len());
        unsafe { self.inner.data.get_unchecked_mut(pointer.index) }
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
    type Item = ReadItem<'a, T>;
    type IntoIter = ReadIter<'a, T>;
    fn into_iter(self) -> Self::IntoIter {
        ReadIter {
            storage: &self.inner,
            skip_lost: false,
            index: 0,
        }
    }
}

impl<'a, T> IntoIterator for &'a mut Storage<T> {
    type Item = WriteItem<'a, T>;
    type IntoIter = WriteIter<'a, T>;
    fn into_iter(self) -> Self::IntoIter {
        WriteIter {
            storage: &mut self.inner,
            skip_lost: false,
            index: 0,
        }
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

    /// Process the pending refcount changes.
    fn sync<F, U>(&mut self, mut fun: F) -> U where
        F: FnMut(&mut Pending) -> U
    {
        let mut pending = self.pending.lock().unwrap();
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
        // other stuff
        fun(&mut *pending)
    }

    /// Synchronize for all the pending updates.
    pub fn sync_pending(&mut self) {
        self.sync(|_| ());
    }

    /// Iterate all components in this storage.
    #[inline]
    pub fn iter(&self) -> ReadIter<T> {
        ReadIter {
            storage: &self.inner,
            skip_lost: false,
            index: 0,
        }
    }

    /// Iterate all components that are still referenced by something.
    #[inline]
    pub fn iter_alive(&self) -> ReadIter<T> {
        ReadIter {
            storage: &self.inner,
            skip_lost: true,
            index: 0,
        }
    }

    /// Iterate all components in this storage, mutably.
    #[inline]
    pub fn iter_mut(&mut self) -> WriteIter<T> {
        WriteIter {
            storage: &mut self.inner,
            skip_lost: false,
            index: 0,
        }
    }

    /// Iterate all components that are still referenced by something, mutably.
    #[inline]
    pub fn iter_alive_mut(&mut self) -> WriteIter<T> {
        self.sync_pending();
        WriteIter {
            storage: &mut self.inner,
            skip_lost: true,
            index: 0,
        }
    }

    /// Pin an iterated item with a newly created `Pointer`.
    pub fn pin(&self, item: &ReadItem<T>) -> Pointer<T> {
        let mut pending = self.pending.lock().unwrap();
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

    /// Pin an mutably iterated item with a newly created `Pointer`.
    pub fn pin_mut(&mut self, item: &WriteItem<T>) -> Pointer<T> {
        let epoch = self.sync(|pending| {
            pending.add_ref.push(item.index);
            pending.get_epoch(item.index)
        });
        Pointer {
            data: PointerData::new(
                item.index,
                epoch,
                self.id,
            ),
            pending: self.pending.clone(),
            marker: PhantomData,
        }
    }

    /// Get a `Pointer` to the first element of the storage.
    pub fn first(&mut self) -> Option<Pointer<T>> {
        let epoch = self.sync(|pending| pending.epoch[0]);
        match self.inner.meta.first_mut() {
            Some(meta) => {
                *meta += 1;
                Some(Pointer {
                    data: PointerData::new(
                        0,
                        epoch,
                        self.id,
                    ),
                    pending: self.pending.clone(),
                    marker: PhantomData,
                })
            },
            None => None,
        }
    }

    /// Move the `Pointer` to the next element, if any.
    pub fn advance(&mut self, mut pointer: Pointer<T>) -> Option<Pointer<T>> {
        debug_assert_eq!(self.id, pointer.data.get_storage_id());
        if pointer.data.get_index()+1 >= self.inner.meta.len() {
            // pointer is dropped here
            return None
        }
        self.inner.meta[pointer.data.get_index()] -= 1;
        let index = pointer.data.get_index();
        pointer.data.set_index(index + 1);
        self.inner.meta[pointer.data.get_index()] += 1;
        //Note: this is unfortunate
        let epoch = self.sync(|pending| pending.epoch[pointer.data.get_index()]);
        pointer.data.set_epoch(epoch);
        Some(pointer)
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
                debug_assert_eq!(self.inner.data.len(), self.inner.meta.len());
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

impl<T> Clone for Pointer<T> {
    #[inline]
    fn clone(&self) -> Pointer<T> {
        self.pending.lock().unwrap().add_ref.push(self.data.get_index());
        Pointer {
            data: self.data,
            pending: self.pending.clone(),
            marker: PhantomData,
        }
    }
}

impl<T> Clone for WeakPointer<T> {
    #[inline]
    fn clone(&self) -> WeakPointer<T> {
        WeakPointer {
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

impl<T> PartialEq for WeakPointer<T> {
    #[inline]
    fn eq(&self, other: &WeakPointer<T>) -> bool {
        self.data == other.data
    }
}

impl<T> Drop for Pointer<T> {
    #[inline]
    fn drop(&mut self) {
        self.pending.lock().unwrap().sub_ref.push(self.data.get_index());
    }
}


/// The item of `ReadIter`.
pub struct ReadItem<'a, T: 'a> {
    value: &'a T,
    index: Index,
}

impl<'a, T> ops::Deref for ReadItem<'a, T> {
    type Target = T;
    fn deref(&self) -> &T {
        self.value
    }
}

impl<'a, T> Iterator for ReadIter<'a, T> {
    type Item = ReadItem<'a, T>;
    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let id = self.index;
            if id == self.storage.data.len() {
                return None
            }
            self.index += 1;
            if !self.skip_lost || self.storage.meta[id] != 0 {
                return Some(ReadItem {
                    value: &self.storage.data[id],
                    index: id,
                })
            }
        }
    }
}


/// The item of `WriteIter`.
pub struct WriteItem<'a, T: 'a> {
    base: *mut T,
    index: Index,
    marker: PhantomData<&'a mut T>,
}

impl<'a, T> ops::Deref for WriteItem<'a, T> {
    type Target = T;
    fn deref(&self) -> &T {
        unsafe{ & *self.base.offset(self.index as isize) }
    }
}

impl<'a, T> ops::DerefMut for WriteItem<'a, T> {
    fn deref_mut(&mut self) -> &mut T {
        unsafe{ &mut *self.base.offset(self.index as isize) }
    }
}

impl<'a, T> Iterator for WriteIter<'a, T> {
    type Item = WriteItem<'a, T>;
    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let id = self.index;
            if id == self.storage.data.len() {
                return None
            }
            self.index += 1;
            if !self.skip_lost || self.storage.meta[id] != 0 {
                return Some(WriteItem {
                    base: self.storage.data.as_mut_ptr(),
                    index: id,
                    marker: PhantomData,
                })
            }
        }
    }
}
