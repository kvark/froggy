use std::marker::PhantomData;
use {DeadComponentError, PendingRef, Pointer, PointerData};

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
/// You will need to [`upgrade`](weak/struct.WeakPointer.html#method.upgrade) `WeakPointer` to access component in storage
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

/// Creates a new `WeakPointer` from the `Pointer`.
#[inline]
pub fn from_pointer<T>(pointer: &Pointer<T>) -> WeakPointer<T> {
    WeakPointer {
        data: pointer.data,
        pending: pointer.pending.clone(),
        marker: PhantomData,
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
