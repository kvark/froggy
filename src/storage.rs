use spin::Mutex;

use std::{
    iter::FromIterator,
    ops, slice,
    sync::{atomic::Ordering, Arc},
};

use crate::{
    Cursor, Epoch, Item, Iter, IterMut, Pending, PendingRef, PhantomData, Pointer, PointerData,
    RefCount, Slice, StorageId, STORAGE_UID,
};

/// Inner storage data that is locked by `RwLock`.
#[derive(Debug)]
pub(crate) struct StorageInner<T> {
    pub(crate) data: Vec<T>,
    pub(crate) meta: Vec<RefCount>,
    free_list: Vec<PointerData>,
}

impl<T> StorageInner<T> {
    pub(crate) fn split(&mut self, offset: PointerData) -> (Slice<T>, &mut T, Slice<T>) {
        let sid = offset.get_storage_id();
        let index = offset.get_index();
        let (left, temp) = self.data.split_at_mut(index as usize);
        let (cur, right) = temp.split_at_mut(1);
        (
            Slice {
                slice: left,
                offset: PointerData::new(0, 0, sid),
            },
            unsafe { cur.get_unchecked_mut(0) },
            Slice {
                slice: right,
                offset: PointerData::new(index + 1, 0, sid),
            },
        )
    }
}

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
    fn from_iter<I>(iter: I) -> Self
    where
        I: IntoIterator<Item = T>,
    {
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
                data,
                meta,
                free_list: Vec::new(),
            },
            pending: Arc::new(Mutex::new(Pending {
                add_ref: Vec::new(),
                sub_ref: Vec::new(),
                epoch,
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
            Vec::with_capacity(capacity),
        )
    }

    /// Synchronize for all the pending updates.
    /// It will update all reference counters in Storage, so
    /// [`iter_alive`](struct.Storage.html#method.iter_alive) and
    /// [`iter_alive_mut`](struct.Storage.html#method.iter_alive_mut) will return actual information.
    ///
    /// Use this function only if necessary, because it needs to block Storage.
    pub fn sync_pending(&mut self) {
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
            let (refs, epoch) = pending.drain_sub();
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
            data: PointerData::new(item.index, pending.get_epoch(item.index), self.id),
            pending: self.pending.clone(),
            marker: PhantomData,
        }
    }

    /// Split the storage according to the provided pointer, returning
    /// the (left slice, pointed data, right slice) triple, where:
    /// left slice contains all the elements that would be iterated prior to the given one,
    /// right slice contains all the elements that would be iterated after the given one
    pub fn split(&mut self, pointer: &Pointer<T>) -> (Slice<T>, &mut T, Slice<T>) {
        debug_assert_eq!(pointer.data.get_storage_id(), self.id);
        self.inner.split(pointer.data)
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

    /// Returns a cursor to the end of the storage, for backwards streaming iteration.
    #[inline]
    pub fn cursor_end(&mut self) -> Cursor<T> {
        let total = self.inner.data.len();
        Cursor {
            storage: &mut self.inner,
            pending: &self.pending,
            index: total,
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
            }
            None => {
                let i = self.inner.meta.len();
                debug_assert_eq!(self.inner.data.len(), i);
                self.inner.data.push(value);
                self.inner.meta.push(1);
                PointerData::new(i, 0, self.id)
            }
        };
        Pointer {
            data,
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
