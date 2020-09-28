use std::{
    fmt,
    hash::{Hash, Hasher},
    marker::PhantomData,
};

use crate::{Pending, PendingRef, PointerData};

/// The error type which is returned from upgrading
/// [`WeakPointer`](struct.WeakPointer.html).
#[derive(Debug, PartialEq)]
pub struct DeadComponentError;

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
    pub(crate) data: PointerData,
    pub(crate) pending: PendingRef,
    pub(crate) marker: PhantomData<T>,
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
    /// See [`WeakPointer`](pointer/struct.WeakPointer.html)
    #[inline]
    pub fn downgrade(&self) -> WeakPointer<T> {
        WeakPointer {
            data: self.data,
            pending: self.pending.clone(),
            marker: PhantomData,
        }
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

/// Weak variant of `Pointer`.
/// `WeakPointer`s are used to avoid deadlocking when dropping structures with cycled references to each other.
/// In the following example `Storage` will stand in memory even after going out of scope, because there is cyclic referencing between `Node`s
///
/// ```rust
/// # use froggy::{Pointer, Storage};
/// struct Node {
///     next: Option<Pointer<Node>>,
/// }
/// # let mut storage = Storage::new();
/// let ptr1 = storage.create(Node { next: None });
/// let ptr2 = storage.create(Node { next: Some(ptr1.clone()) });
/// storage[&ptr1].next = Some(ptr2.clone());
/// ```
///
/// To avoid such situations, just replace `Option<Pointer<Node>>` with `Option<WeakPointer<Node>>`
/// # Example
///
/// ```rust
/// # let mut storage = froggy::Storage::new();
/// let pointer = storage.create(1i32);
/// // create WeakPointer to this component
/// let weak = pointer.downgrade();
/// ```
///
/// You will need to [`upgrade`](struct.WeakPointer.html#method.upgrade) `WeakPointer` to access component in storage
///
/// ```rust
/// # fn try_main() -> Result<(), froggy::DeadComponentError> {
/// # let mut storage = froggy::Storage::new();
/// # let _pointer = storage.create(1i32);
/// # let weak = _pointer.downgrade();
/// let pointer = weak.upgrade()?;
/// storage[&pointer] = 20;
/// # Ok(()) }
/// # fn main() { try_main().unwrap(); }
/// ```
#[derive(Debug)]
pub struct WeakPointer<T> {
    data: PointerData,
    pending: PendingRef,
    marker: PhantomData<T>,
}

impl<T> WeakPointer<T> {
    /// Upgrades the `WeakPointer` to a `Pointer`, if possible.
    /// # Errors
    /// Returns [`DeadComponentError`](struct.DeadComponentError.html) if the related component in storage was destroyed.
    pub fn upgrade(&self) -> Result<Pointer<T>, DeadComponentError> {
        let mut pending = self.pending.lock();
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

impl<T> PartialEq for WeakPointer<T> {
    #[inline]
    fn eq(&self, other: &WeakPointer<T>) -> bool {
        self.data == other.data
    }
}

impl<T> Eq for WeakPointer<T> {}
