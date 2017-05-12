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

use std::marker::PhantomData;
use std::ops;
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicUsize, Ordering, ATOMIC_USIZE_INIT};
use std::vec::Drain;

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
    free_list: Vec<(usize, Epoch)>,
}

/// Pending reference counts updates.
#[derive(Debug)]
struct Pending {
    add_ref: Vec<usize>,
    sub_ref: Vec<usize>,
    epoch: Vec<Epoch>,
}

impl Pending {
    #[inline]
    fn drain_sub(&mut self) -> (Drain<usize>, &mut [Epoch]) {
        (self.sub_ref.drain(..), self.epoch.as_mut_slice())
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
    index: usize,
    epoch: Epoch,
    storage_id: StorageId,
    pending: PendingRef,
    marker: PhantomData<T>,
}

impl<T> Pointer<T> {
    /// Creates a new `WeakPointer` to this component.
    #[inline]
    pub fn downgrade(&self) -> WeakPointer<T> {
        WeakPointer {
            index: self.index,
            epoch: self.epoch,
            storage_id: self.storage_id,
            pending: self.pending.clone(),
            marker: PhantomData,
        }
    }
}

/// Weak variant of `Pointer`.
pub struct WeakPointer<T> {
    index: usize,
    epoch: Epoch,
    storage_id: StorageId,
    pending: PendingRef,
    marker: PhantomData<T>,
}

impl<T> WeakPointer<T> {
    /// Upgrades the `WeakPointer` to a `Pointer`, if possible.
    /// Returns `Err` if the strong count has reached zero or the inner value was destroyed.
    pub fn upgrade(&self) -> Result<Pointer<T>, DeadComponentError> {
        let mut pending = self.pending.lock().unwrap();
        if pending.epoch[self.index] != self.epoch {
            return Err(DeadComponentError);
        }
        pending.add_ref.push(self.index);
        Ok(Pointer {
            index: self.index,
            epoch: self.epoch,
            storage_id: self.storage_id,
            pending: self.pending.clone(),
            marker: PhantomData,
        })
    }
}

/// Read lock on the storage, allows multiple clients to read from the same storage simultaneously.
pub struct ReadLock<'a, T: 'a> {
    storage: &'a StorageInner<T>,
    storage_id: StorageId,
    pending: PendingRef,
}

/// Iterator for reading components.
pub struct ReadIter<'a, T: 'a> {
    lock: &'a ReadLock<'a, T>,
    skip_lost: bool,
    index: usize,
}

/// Write lock on the storage allows exclusive access.
pub struct WriteLock<'a, T: 'a> {
    storage: &'a mut StorageInner<T>,
    storage_id: StorageId,
    pending: PendingRef,
}

/// Iterator for writing components.
pub struct WriteIter<'b, 'a: 'b, T: 'a> {
    lock: &'b mut WriteLock<'a, T>,
    skip_lost: bool,
    index: usize,
}


impl<T> Storage<T> {
    fn from_inner(inner: StorageInner<T>) -> Storage<T> {
        let uid = STORAGE_UID.fetch_add(1, Ordering::Relaxed) as u8;
        let pending = Pending {
            add_ref: Vec::new(),
            sub_ref: Vec::new(),
            epoch: Vec::new(),
        };
        Storage {
            inner: inner,
            pending: Arc::new(Mutex::new(pending)),
            id: uid,
        }
    }

    /// Create a new empty storage.
    pub fn new() -> Storage<T> {
        Self::from_inner(StorageInner {
            data: Vec::new(),
            meta: Vec::new(),
            free_list: Vec::new(),
        })
    }

    /// Create a new empty storage with specified capacity.
    pub fn with_capacity(capacity: usize) -> Storage<T> {
        Self::from_inner(StorageInner {
            data: Vec::with_capacity(capacity),
            meta: Vec::with_capacity(capacity),
            free_list: Vec::new(),
        })
    }

    /// Lock the storage for reading. This operation will block until the write locks are done.
    #[inline]
    pub fn read(&self) -> ReadLock<T> {
        ReadLock {
            storage: &self.inner,
            storage_id: self.id,
            pending: self.pending.clone(),
        }
    }

    /// Lock the storage for writing. This operation will block untill all the locks are done.
    pub fn write(&mut self) -> WriteLock<T> {
        // process the pending refcount changes
        let mut pending = self.pending.lock().unwrap();
        for index in pending.add_ref.drain(..) {
            self.inner.meta[index] += 1;
        }
        let (refs, mut epoch) = pending.drain_sub();
        for index in refs {
            self.inner.meta[index] -= 1;
            if self.inner.meta[index] == 0 {
                epoch[index] += 1;
                self.inner.free_list.push((index, epoch[index]));
            }
        }
        // return the lock
        WriteLock {
            storage: &mut self.inner,
            storage_id: self.id,
            pending: self.pending.clone(),
        }
    }
}

impl<T> Clone for Pointer<T> {
    #[inline]
    fn clone(&self) -> Pointer<T> {
        self.pending.lock().unwrap().add_ref.push(self.index);
        Pointer {
            index: self.index,
            epoch: self.epoch,
            storage_id: self.storage_id,
            pending: self.pending.clone(),
            marker: PhantomData,
        }
    }
}

impl<T> Clone for WeakPointer<T> {
    #[inline]
    fn clone(&self) -> WeakPointer<T> {
        WeakPointer {
            index: self.index,
            epoch: self.epoch,
            storage_id: self.storage_id,
            pending: self.pending.clone(),
            marker: PhantomData,
        }
    }
}

impl<T> PartialEq for Pointer<T> {
    #[inline]
    fn eq(&self, other: &Pointer<T>) -> bool {
        self.index == other.index &&
        self.storage_id == other.storage_id
    }
}

impl<T> Drop for Pointer<T> {
    #[inline]
    fn drop(&mut self) {
        self.pending.lock().unwrap().sub_ref.push(self.index);
    }
}


/// The item of `ReadIter`.
pub struct ReadItem<'a, T: 'a> {
    value: &'a T,
    index: usize,
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
            if id == self.lock.storage.data.len() {
                return None
            }
            self.index += 1;
            if !self.skip_lost || self.lock.storage.meta[id] != 0 {
                return Some(ReadItem {
                    value: &self.lock.storage.data[id],
                    index: id,
                })
            }
        }
    }
}

impl<'a, 'b, T> ops::Index<&'b Pointer<T>> for ReadLock<'a, T> {
    type Output = T;
    #[inline]
    fn index(&self, pointer: &'b Pointer<T>) -> &T {
        debug_assert_eq!(self.storage_id, pointer.storage_id);
        debug_assert!(pointer.index < self.storage.data.len());
        unsafe { self.storage.data.get_unchecked(pointer.index) }
    }
}

impl<'a, T> ReadLock<'a, T> {
    /// Iterate all components in this locked storage.
    #[inline]
    pub fn iter(&'a self) -> ReadIter<'a, T> {
        ReadIter {
            lock: self,
            skip_lost: false,
            index: 0,
        }
    }

    /// Iterate all components that are still referenced by something.
    #[inline]
    pub fn iter_alive(&'a self) -> ReadIter<'a, T> {
        ReadIter {
            lock: self,
            skip_lost: true,
            index: 0,
        }
    }

    /// Pin an iterated item with a newly created `Pointer`.
    pub fn pin(&self, item: &ReadItem<'a, T>) -> Pointer<T> {
        let mut pending = self.pending.lock().unwrap();
        pending.add_ref.push(item.index);
        Pointer {
            index: item.index,
            epoch: pending.epoch[item.index],
            storage_id: self.storage_id,
            pending: self.pending.clone(),
            marker: PhantomData,
        }
    }
}


/// The item of `WriteIter`.
pub struct WriteItem<'a, T: 'a> {
    base: *mut T,
    index: usize,
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

impl<'b, 'a, T> Iterator for WriteIter<'b, 'a, T> {
    type Item = WriteItem<'a, T>;
    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let id = self.index;
            if id == self.lock.storage.data.len() {
                return None
            }
            self.index += 1;
            if !self.skip_lost || self.lock.storage.meta[id] != 0 {
                return Some(WriteItem {
                    base: self.lock.storage.data.as_mut_ptr(),
                    index: id,
                    marker: PhantomData,
                })
            }
        }
    }
}

impl<'a, 'b, T> ops::Index<&'b Pointer<T>> for WriteLock<'a, T> {
    type Output = T;
    #[inline]
    fn index(&self, pointer: &'b Pointer<T>) -> &T {
        debug_assert_eq!(self.storage_id, pointer.storage_id);
        debug_assert!(pointer.index < self.storage.data.len());
        unsafe { self.storage.data.get_unchecked(pointer.index) }
    }
}

impl<'a, 'b, T> ops::IndexMut<&'b Pointer<T>> for WriteLock<'a, T> {
    #[inline]
    fn index_mut(&mut self, pointer: &'b Pointer<T>) -> &mut T {
        debug_assert_eq!(self.storage_id, pointer.storage_id);
        debug_assert!(pointer.index < self.storage.data.len());
        unsafe { self.storage.data.get_unchecked_mut(pointer.index) }
    }
}

impl<'a, T> WriteLock<'a, T> {
    /// Iterate all components in this locked storage.
    #[inline]
    pub fn iter<'b>(&'b mut self) -> WriteIter<'b, 'a, T> {
        WriteIter {
            lock: self,
            skip_lost: false,
            index: 0,
        }
    }

    /// Iterate all components that are still referenced by something.
    #[inline]
    pub fn iter_alive<'b>(&'b mut self) -> WriteIter<'b, 'a, T> {
        WriteIter {
            lock: self,
            skip_lost: true,
            index: 0,
        }
    }

    /// Pin an iterated item with a newly created `Pointer`.
    pub fn pin(&mut self, item: &WriteItem<'a, T>) -> Pointer<T> {
        let mut pending = self.pending.lock().unwrap();
        pending.add_ref.push(item.index);
        Pointer {
            index: item.index,
            epoch: pending.epoch[item.index],
            storage_id: self.storage_id,
            pending: self.pending.clone(),
            marker: PhantomData,
        }
    }

    /// Get a `Pointer` to the first element of the storage.
    pub fn first(&mut self) -> Option<Pointer<T>> {
        match self.storage.meta.first_mut() {
            Some(meta) => {
                *meta += 1;
                Some(Pointer {
                    index: 0,
                    epoch: self.pending.lock().unwrap().epoch[0],
                    storage_id: self.storage_id,
                    pending: self.pending.clone(),
                    marker: PhantomData,
                })
            },
            None => None,
        }
    }

    /// Move the `Pointer` to the next element, if any.
    pub fn advance(&mut self, mut pointer: Pointer<T>) -> Option<Pointer<T>> {
        debug_assert_eq!(self.storage_id, pointer.storage_id);
        if pointer.index+1 >= self.storage.meta.len() {
            // pointer is dropped here
            return None
        }
        self.storage.meta[pointer.index] -= 1;
        pointer.index += 1;
        let pending = self.pending.lock().unwrap(); //Note: this is unfortunate
        self.storage.meta[pointer.index] += 1;
        pointer.epoch = pending.epoch[pointer.index];
        Some(pointer)
    }

    /// Add a new component to the storage, returning the `Pointer` to it.
    pub fn create(&mut self, value: T) -> Pointer<T> {
        let (index, epoch) = match self.storage.free_list.pop() {
            Some((i, e)) => {
                debug_assert_eq!(self.storage.meta[i], 0);
                self.storage.data[i] = value;
                self.storage.meta[i] = 1;
                (i, e)
            },
            None => {
                debug_assert_eq!(self.storage.data.len(), self.storage.meta.len());
                self.storage.data.push(value);
                self.storage.meta.push(1);
                self.pending.lock().unwrap().epoch.push(0);
                (self.storage.meta.len() - 1, 0)
            },
        };
        Pointer {
            index: index,
            epoch: epoch,
            storage_id: self.storage_id,
            pending: self.pending.clone(),
            marker: PhantomData,
        }
    }
}
