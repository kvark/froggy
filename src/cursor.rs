use std::{marker::PhantomData, ops};

use crate::{Index, PendingRef, Pointer, PointerData, StorageId, StorageInner};

/// A slice of a storage. Useful for cursor iteration.
#[derive(Debug)]
pub struct Slice<'a, T: 'a> {
    pub(crate) slice: &'a mut [T],
    pub(crate) offset: PointerData,
}

impl<'a, T> Slice<'a, T> {
    /// Check if the slice contains no elements.
    pub fn is_empty(&self) -> bool {
        self.slice.is_empty()
    }

    /// Get a reference by pointer. Returns None if an element
    /// is outside of the slice.
    pub fn get(&'a self, pointer: &Pointer<T>) -> Option<&'a T> {
        debug_assert_eq!(pointer.data.get_storage_id(), self.offset.get_storage_id());
        let index = pointer
            .data
            .get_index()
            .wrapping_sub(self.offset.get_index());
        self.slice.get(index as usize)
    }

    /// Get a mutable reference by pointer. Returns None if an element
    /// is outside of the slice.
    pub fn get_mut(&'a mut self, pointer: &Pointer<T>) -> Option<&'a mut T> {
        debug_assert_eq!(pointer.data.get_storage_id(), self.offset.get_storage_id());
        let index = pointer
            .data
            .get_index()
            .wrapping_sub(self.offset.get_index());
        self.slice.get_mut(index as usize)
    }
}

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
/// with Pointer.
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
/// while let Some((left, mut item, _)) = cursor.next() {
///    // let's look for the other Node
///    match item.pointer {
///        Some(ref pointer) => {
///            let ref pointer = pointer.upgrade()?;
///            if let Some(ref other_node) = left.get(pointer) {
///                do_something(other_node);
///            }
///        },
///        None => {},
///    }
/// }
/// # Ok(())
/// # }
/// # fn main() { try_main(); }
/// ```
#[derive(Debug)]
pub struct CursorItem<'a, T: 'a> {
    item: &'a mut T,
    pending: &'a PendingRef,
    data: PointerData,
}

impl<'a, T> ops::Deref for CursorItem<'a, T> {
    type Target = T;
    fn deref(&self) -> &T {
        self.item
    }
}

impl<'a, T> ops::DerefMut for CursorItem<'a, T> {
    fn deref_mut(&mut self) -> &mut T {
        self.item
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
}

/// Streaming iterator providing mutable components
/// and a capability to look back/ahead.
///
/// See documentation of [`CursorItem`](struct.CursorItem.html).
#[derive(Debug)]
pub struct Cursor<'a, T: 'a> {
    pub(crate) storage: &'a mut StorageInner<T>,
    pub(crate) pending: &'a PendingRef,
    pub(crate) index: Index,
    pub(crate) storage_id: StorageId,
}

impl<'a, T> Cursor<'a, T> {
    fn split(&mut self, index: usize) -> (Slice<T>, CursorItem<T>, Slice<T>) {
        let data = PointerData::new(index, 0, self.storage_id);
        let (left, item, right) = self.storage.split(data);
        let item = CursorItem {
            item,
            data,
            pending: self.pending,
        };
        (left, item, right)
    }

    /// Advance the stream to the next item.
    pub fn next(&mut self) -> Option<(Slice<T>, CursorItem<T>, Slice<T>)> {
        loop {
            let id = self.index;
            self.index += 1;
            match self.storage.meta.get(id) {
                None => {
                    self.index = id; // prevent the bump of the index
                    return None;
                }
                Some(&0) => (),
                Some(_) => return Some(self.split(id)),
            }
        }
    }

    /// Advance the stream to the previous item.
    pub fn prev(&mut self) -> Option<(Slice<T>, CursorItem<T>, Slice<T>)> {
        loop {
            if self.index == 0 {
                return None;
            }
            self.index -= 1;
            let id = self.index;
            debug_assert!(id < self.storage.meta.len());
            if *unsafe { self.storage.meta.get_unchecked(id) } != 0 {
                return Some(self.split(id));
            }
        }
    }
}
