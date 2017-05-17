use ::{Pointer, PointerData, PendingRef, DeadComponentError};
use ::std::marker::PhantomData;

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