use std::ops;
use std::sync::{Arc, Mutex, RwLock, RwLockReadGuard, RwLockWriteGuard};


type RefCount = u16;

pub struct StorageInner<T> {
    data: Vec<T>,
    meta: Vec<RefCount>,
    free_list: Vec<usize>,
}

type FreeListRef = Arc<Mutex<Vec<usize>>>;
type StorageRef<T> = Arc<RwLock<StorageInner<T>>>;
pub struct Storage<T>(StorageRef<T>, FreeListRef);

pub struct Pointer<T> {
    index: usize,
    target: StorageRef<T>,
    pending: FreeListRef,
}

pub struct ReadLock<'a, T: 'a> {
    guard: RwLockReadGuard<'a, StorageInner<T>>,
    storage: StorageRef<T>,
}

pub struct WriteLock<'a, T: 'a> {
    guard: RwLockWriteGuard<'a, StorageInner<T>>,
    storage: StorageRef<T>,
    pending: FreeListRef,
}


impl<T> Storage<T> {
    /// Create a new empty storage.
    pub fn new() -> Storage<T> {
        let inner = StorageInner {
            data: Vec::new(),
            meta: Vec::new(),
            free_list: Vec::new(),
        };
        let pending = Vec::new();
        Storage(Arc::new(RwLock::new(inner)), Arc::new(Mutex::new(pending)))
    }

    pub fn read(&self) -> ReadLock<T> {
        ReadLock {
            guard: self.0.read().unwrap(),
            storage: self.0.clone(),
        }
    }

    pub fn write(&mut self) -> WriteLock<T> {
        let mut s = self.0.write().unwrap();
        // process the pending frees
        let mut pending = self.1.lock().unwrap();
        for index in pending.drain(..) {
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
        self.pending.lock().unwrap().push(self.index);
    }
}


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
                self.guard.data.len() - 1
            },
        };
        Pointer {
            index: index,
            target: self.storage.clone(),
            pending: self.pending.clone(),
        }
    }
}
