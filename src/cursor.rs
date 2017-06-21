use {Cursor, NotFoundError, PendingRef, Pointer, PointerData};
use std::marker::PhantomData;
use std::ops;

/// Item of the streaming iterator.
///
/// [`Cursor`](struct.Cursor.html) and `CursorItem` are extremely useful
/// when you need to iterate over [`Pointers`](struct.Pointer.html) in storage
/// with ability to get other component in the same storage during iterating.
///
/// # Examples
/// Unfortunately, you can't use common `for` loop,
/// but you can use `while let` statement:
///
/// ```rust
/// # let mut storage: froggy::Storage<i32> = froggy::Storage::new();
/// let mut cursor = storage.cursor();
/// while let Some(item) = cursor.next() {
///    // ... your code
/// }
///
/// ```
/// While iterating, you can [`pin`](struct.CursorItem.html#method.pin) item
/// with Pointer. Also you can [`look_ahead`](struct.CursorItem.html#method.look_ahead)
/// and [`look_back`](struct.CursorItem.html#method.look_back) for other items
/// in this storage.
///
/// ```rust
/// # use froggy::WeakPointer;
/// #[derive(Debug, PartialEq)]
/// struct Node {
///    pointer: Option<WeakPointer<Node>>,
/// }
/// //...
/// # fn do_something(_: &Node) {}
/// # fn try_main() -> Result<(), froggy::DeadComponentError> {
/// # let mut storage = froggy::Storage::new();
/// # let ptr1 = storage.create(Node { pointer: None });
/// # let ptr2 = storage.create(Node { pointer: None });
/// # storage[&ptr1].pointer = Some(ptr2.downgrade());
/// # storage[&ptr2].pointer = Some(ptr1.downgrade());
/// let mut cursor = storage.cursor();
/// while let Some(mut item) = cursor.next() {
///    // let's look for the other Node
///    match item.pointer {
///        Some(ref pointer) => {
///            let ref pointer = pointer.upgrade()?;
///            if let Ok(ref other_node) = item.look_back(pointer) {
///                do_something(other_node);
///            }
///        },
///        None => {},
///    }
/// }
/// # Ok(())
/// # }
/// # fn main() { try_main(); }
#[derive(Debug)]
pub struct CursorItem<'a, T: 'a> {
    slice: &'a mut [T],
    pending: &'a PendingRef,
    data: PointerData,
}

impl<'a, T> ops::Deref for CursorItem<'a, T> {
    type Target = T;
    fn deref(&self) -> &T {
        unsafe{ self.slice.get_unchecked(self.data.get_index()) }
    }
}

impl<'a, T> ops::DerefMut for CursorItem<'a, T> {
    fn deref_mut(&mut self) -> &mut T {
        unsafe{ self.slice.get_unchecked_mut(self.data.get_index()) }
    }
}

impl<'a, T> CursorItem<'a, T> {
    /// Pin the item with a strong pointer.
    pub fn pin(&self) -> Pointer<T> {
        let epoch = {
            let mut pending = self.pending.lock();
            pending.add_ref.push(self.data.get_index());
            pending.get_epoch(self.data.get_index())
        };
        Pointer {
            data: self.data.with_epoch(epoch),
            pending: self.pending.clone(),
            marker: PhantomData,
        }
    }

    /// Attempt to read an element before the cursor by a pointer.
    ///
    /// # Errors
    /// Returns [`NotFoundError`](struct.NotFoundError.html) if there is no component
    /// before current `CursorItem` in the `Storage`.
    pub fn look_back(&self, pointer: &'a Pointer<T>) -> Result<&T, NotFoundError> {
        debug_assert_eq!(pointer.data.get_storage_id(), self.data.get_storage_id());
        let id = pointer.data.get_index();
        if id < self.data.get_index() {
            Ok(unsafe { self.slice.get_unchecked(id) })
        } else {
            Err(NotFoundError)
        }
    }

    /// Attempt to mutate an element before the cursor by a pointer.
    ///
    /// # Errors
    /// Returns [`NotFoundError`](struct.NotFoundError.html) if there is no component
    /// before current `CursorItem` in the `Storage`.
    pub fn look_back_mut(&mut self, pointer: &'a Pointer<T>) -> Result<&mut T, NotFoundError> {
        debug_assert_eq!(pointer.data.get_storage_id(), self.data.get_storage_id());
        let id = pointer.data.get_index();
        if id < self.data.get_index() {
            Ok(unsafe { self.slice.get_unchecked_mut(id) })
        } else {
            Err(NotFoundError)
        }
    }

    /// Attempt to read an element after the cursor by a pointer.
    ///
    /// # Errors
    /// Returns [`NotFoundError`](struct.NotFoundError.html) if there is no component
    /// after current `CursorItem` in the `Storage`.
    pub fn look_ahead(&self, pointer: &'a Pointer<T>) -> Result<&T, NotFoundError> {
        debug_assert_eq!(pointer.data.get_storage_id(), self.data.get_storage_id());
        let id = pointer.data.get_index();
        if id > self.data.get_index() {
            debug_assert!(id < self.slice.len());
            Ok(unsafe { self.slice.get_unchecked(id) })
        } else {
            Err(NotFoundError)
        }
    }

    /// Attempt to mutate an element after the cursor by a pointer.
    ///
    /// # Errors
    /// Returns [`NotFoundError`](struct.NotFoundError.html) if there is no component
    /// after current `CursorItem` in the `Storage`.
    pub fn look_ahead_mut(&mut self, pointer: &'a Pointer<T>) -> Result<&mut T, NotFoundError> {
        debug_assert_eq!(pointer.data.get_storage_id(), self.data.get_storage_id());
        let id = pointer.data.get_index();
        if id > self.data.get_index() {
            debug_assert!(id < self.slice.len());
            Ok(unsafe { self.slice.get_unchecked_mut(id) })
        } else {
            Err(NotFoundError)
        }
    }
}

impl<'a, T> Cursor<'a, T> {
    /// Advance the stream to the next item.
    pub fn next(&mut self) -> Option<CursorItem<T>> {
        loop {
            let id = self.index;
            if id >= self.storage.data.len() {
                return None
            }
            self.index += 1;
            if !self.skip_lost || unsafe {*self.storage.meta.get_unchecked(id)} != 0 {
                return Some(CursorItem {
                    slice: &mut self.storage.data,
                    data: PointerData::new(id, 0, self.storage_id),
                    pending: self.pending,
                })
            }
        }
    }
}
