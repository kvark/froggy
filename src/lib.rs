use std::ops;
use std::sync::{Arc, Mutex, RwLock, RwLockReadGuard, RwLockWriteGuard};


type RefCount = u16;

struct StorageInner<T> {
    data: Vec<T>,
    meta: Vec<RefCount>,
    free_list: Vec<usize>,
}

struct Pending {
    add_ref: Vec<usize>,
    sub_ref: Vec<usize>,
}

type StorageRef<T> = Arc<RwLock<StorageInner<T>>>;
type PendingRef = Arc<Mutex<Pending>>;
pub struct Storage<T>(StorageRef<T>, PendingRef);

pub struct Pointer<T> {
    index: usize,
    target: StorageRef<T>,
    pending: PendingRef,
}

pub struct ReadLock<'a, T: 'a> {
    guard: RwLockReadGuard<'a, StorageInner<T>>,
    storage: StorageRef<T>,
    pending: PendingRef,
}

pub struct WriteLock<'a, T: 'a> {
    guard: RwLockWriteGuard<'a, StorageInner<T>>,
    storage: StorageRef<T>,
    pending: PendingRef,
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

    pub fn read(&self) -> ReadLock<T> {
        ReadLock {
            guard: self.0.read().unwrap(),
            storage: self.0.clone(),
            pending: self.1.clone(),
        }
    }

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


// Warning: this exposes deleted entries
impl<'a, T> ops::Deref for ReadLock<'a, T> {
    type Target = [T];
    fn deref(&self) -> &Self::Target {
        &self.guard.data
    }
}

impl<'a, T> ReadLock<'a, T> {
    pub fn access(&self, ptr: &Pointer<T>) -> &T {
        debug_assert_eq!(&*self.storage as *const _, &*ptr.target as *const _);
        &self.guard.data[ptr.index]
    }

    pub fn pin(&self, index: usize) -> Option<Pointer<T>> {
        if index < self.guard.data.len() && self.guard.meta[index] != 0 {
            self.pending.lock().unwrap().add_ref.push(index);
            Some(Pointer {
                index: index,
                target: self.storage.clone(),
                pending: self.pending.clone(),
            })
        } else {
            None
        }
    }
}

// Warning: this exposes deleted entries
impl<'a, T> ops::Deref for WriteLock<'a, T> {
    type Target = [T];
    fn deref(&self) -> &Self::Target {
        &self.guard.data
    }
}

// Warning: this exposes deleted entries
impl<'a, T> ops::DerefMut for WriteLock<'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.guard.data
    }
}

impl<'a, T> WriteLock<'a, T> {
    pub fn access(&mut self, ptr: &Pointer<T>) -> &mut T {
        debug_assert_eq!(&*self.storage as *const _, &*ptr.target as *const _);
        &mut self.guard.data[ptr.index]
    }

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
