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
use std::{mem, ops};
use std::sync::{Arc, Mutex, RwLock, RwLockReadGuard, RwLockWriteGuard};

/// Reference counter type. It doesn't make sense to allocate too much bit for it in regular applications.
// TODO: control by a cargo feature
type RefCount = u16;

/// Inner storage data that is locked by `RwLock`.
struct StorageInner<T> {
    data: Vec<T>,
    meta: Vec<RefCount>,
    free_list: Vec<usize>,
}

/// Pending reference counts updates.
struct Pending {
    add_ref: Vec<usize>,
    sub_ref: Vec<usize>,
}

/// Shared pointer to the inner storage.
type StorageRef<T> = Arc<RwLock<StorageInner<T>>>;
/// Shared pointer to the pending updates.
type PendingRef = Arc<Mutex<Pending>>;
/// Component storage type.
/// Manages the components and allows for efficient processing.
pub struct Storage<T>(StorageRef<T>, PendingRef);

/// A pointer to a component of type `T`.
/// The component is guaranteed to be accessible for as long as this pointer is alive.
/// You'd need a locked storage to access the data.
/// The pointer also holds the storage alive and knows the index of the element to look up.
pub struct Pointer<T> {
    index: usize,
    target: StorageRef<T>,
    pending: PendingRef,
}


/// Read lock on the storage, allows multiple clients to read from the same storage simultaneously.
pub struct ReadLock<'a, T: 'a> {
    guard: RwLockReadGuard<'a, StorageInner<T>>,
    storage: StorageRef<T>,
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
    guard: RwLockWriteGuard<'a, StorageInner<T>>,
    storage: StorageRef<T>,
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
        let pending = Pending {
            add_ref: Vec::new(),
            sub_ref: Vec::new(),
        };
        Storage(Arc::new(RwLock::new(inner)),
                Arc::new(Mutex::new(pending)))
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
    pub fn read(&self) -> ReadLock<T> {
        ReadLock {
            guard: self.0.read().unwrap(),
            storage: self.0.clone(),
            pending: self.1.clone(),
        }
    }

    /// Lock the storage for writing. This operation will block untill all the locks are done.
    pub fn write(&self) -> WriteLock<T> {
        let mut s = self.0.write().unwrap();
        // process the pending refcount changes
        let mut pending = self.1.lock().unwrap();
        for index in pending.add_ref.drain(..) {
            s.meta[index] += 1;
        }
        for index in pending.sub_ref.drain(..) {
            s.meta[index] -= 1;
            if s.meta[index] == 0 {
                s.free_list.push(index);
            }
        }
        // return the lock
        WriteLock {
            guard: s,
            storage: self.0.clone(),
            pending: self.1.clone(),
        }
    }
}

impl<T> Clone for Pointer<T> {
    fn clone(&self) -> Pointer<T> {
        self.pending.lock().unwrap().add_ref.push(self.index);
        Pointer {
            index: self.index,
            target: self.target.clone(),
            pending: self.pending.clone(),
        }
    }
}

impl<T> PartialEq for Pointer<T> {
    fn eq(&self, other: &Pointer<T>) -> bool {
        self.index == other.index &&
        &*self.target as *const _ == &*other.target as *const _
    }
}

impl<T> Drop for Pointer<T> {
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
            if id == self.lock.guard.data.len() {
                return None
            }
            self.index += 1;
            if !self.skip_lost || self.lock.guard.meta[id] != 0 {
                return Some(ReadItem {
                    value: &self.lock.guard.data[id],
                    index: id,
                })
            }
        }
    }
}

impl<'a, T> ReadLock<'a, T> {
    /// Borrow a pointed component for reading.
    pub fn access(&self, ptr: &Pointer<T>) -> &T {
        debug_assert_eq!(&*self.storage as *const _, &*ptr.target as *const _);
        &self.guard.data[ptr.index]
    }

    /// Iterate all components in this locked storage.
    pub fn iter(&'a self) -> ReadIter<'a, T> {
        ReadIter {
            lock: self,
            skip_lost: false,
            index: 0,
        }
    }

    /// Iterate all components that are still referenced by something.
    pub fn iter_alive(&'a self) -> ReadIter<'a, T> {
        ReadIter {
            lock: self,
            skip_lost: true,
            index: 0,
        }
    }

    /// Pin an iterated item with a newly created `Pointer`.
    pub fn pin(&self, item: &ReadItem<'a, T>) -> Pointer<T> {
        self.pending.lock().unwrap().add_ref.push(item.index);
        Pointer {
            index: item.index,
            target: self.storage.clone(),
            pending: self.pending.clone(),
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
            if id == self.lock.guard.data.len() {
                return None
            }
            self.index += 1;
            if !self.skip_lost || self.lock.guard.meta[id] != 0 {
                return Some(WriteItem {
                    base: self.lock.guard.data.as_mut_ptr(),
                    index: id,
                    marker: PhantomData,
                })
            }
        }
    }
}

impl<'a, T> WriteLock<'a, T> {
    /// Borrow a pointed component for writing.
    pub fn access_mut(&mut self, pointer: &Pointer<T>) -> &mut T {
        debug_assert_eq!(&*self.storage as *const _, &*pointer.target as *const _);
        &mut self.guard.data[pointer.index]
    }

    /// Borrow a pointed component for reading.
    pub fn access(&self, pointer: &Pointer<T>) -> &T {
        debug_assert_eq!(&*self.storage as *const _, &*pointer.target as *const _);
        &self.guard.data[pointer.index]
    }

    /// Iterate all components in this locked storage.
    pub fn iter<'b>(&'b mut self) -> WriteIter<'b, 'a, T> {
        WriteIter {
            lock: self,
            skip_lost: false,
            index: 0,
        }
    }

    /// Iterate all components that are still referenced by something.
    pub fn iter_alive<'b>(&'b mut self) -> WriteIter<'b, 'a, T> {
        WriteIter {
            lock: self,
            skip_lost: true,
            index: 0,
        }
    }

    /// Pin an iterated item with a newly created `Pointer`.
    pub fn pin(&mut self, item: &WriteItem<'a, T>) -> Pointer<T> {
        // self.guard.meta[item.index] += 1; // requires mutable borrow
        self.pending.lock().unwrap().add_ref.push(item.index);
        Pointer {
            index: item.index,
            target: self.storage.clone(),
            pending: self.pending.clone(),
        }
    }

    /// Get a `Pointer` to the first element of the storage.
    pub fn first(&mut self) -> Option<Pointer<T>> {
        match self.guard.meta.first_mut() {
            Some(meta) => {
                *meta += 1;
                Some(Pointer {
                    index: 0,
                    target: self.storage.clone(),
                    pending: self.pending.clone(),
                })
            },
            None => None,
        }
    }

    /// Move the `Pointer` to the next element, if any.
    pub fn advance(&mut self, mut pointer: Pointer<T>) -> Option<Pointer<T>> {
        debug_assert_eq!(&*self.storage as *const _, &*pointer.target as *const _);
        self.guard.meta[pointer.index] -= 1;
        pointer.index += 1;
        if pointer.index < self.guard.meta.len() {
            self.guard.meta[pointer.index] += 1;
            Some(pointer)
        } else {
            // the refcount is already updated
            mem::forget(pointer);
            None
        }
    }

    /// Add a new component to the storage, returning the `Pointer` to it.
    pub fn create(&mut self, value: T) -> Pointer<T> {
        let index = match self.guard.free_list.pop() {
            Some(i) => {
                debug_assert_eq!(self.guard.meta[i], 0);
                self.guard.data[i] = value;
                self.guard.meta[i] = 1;
                i
            },
            None => {
                debug_assert_eq!(self.guard.data.len(), self.guard.meta.len());
                self.guard.data.push(value);
                self.guard.meta.push(1);
                self.guard.meta.len() - 1
            },
        };
        Pointer {
            index: index,
            target: self.storage.clone(),
            pending: self.pending.clone(),
        }
    }
}
