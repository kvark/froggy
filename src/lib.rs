/*!
Component Graph System prototype.

Froggy is all about the smart component storage, unambiguously called `Storage`.
Components inside it are automatically reference-counted, and could be referenced by a `Pointer`.
The components are stored linearly, allowing for the efficient bulk data processing.
`Storage` has to be locked temporarily for either read or write before any usage.

You can find more information about Component Graph System concept on the [wiki](https://github.com/kvark/froggy/wiki/Component-Graph-System).
Comparing to Entity-Component Systems (ECS), CGS doesn't have the backwards relation of components to entities.
Thus, it can't process all "entities" by just selecting a subset of components to work on, besides not having the whole "entity" concept.
However, CGS has a number of advantages:

  - you can share components naturally
  - you don't need to care about the component lifetime, it is managed automatically
  - you can have deeper hierarchies of components, with one component referencing the others
  - you can have user structures referencing components freely
  - there are no restrictions on the component types, and no need to implement any traits

*/
#![warn(missing_docs)]
#![doc(html_root_url = "https://docs.rs/froggy/0.4.4")]

use spin::Mutex;
use std::{
    sync::{atomic::AtomicUsize, Arc},
    vec::Drain,
};

mod bitfield;
mod cursor;
mod pointer;
mod storage;

use crate::bitfield::PointerData;
use crate::storage::StorageInner;

pub use crate::cursor::{Cursor, CursorItem, Slice};
pub use crate::pointer::{DeadComponentError, Pointer, WeakPointer};
pub use crate::storage::{Item, Iter, IterMut, Storage};

type Index = usize;

/// Reference counter type. It doesn't make sense to allocate too much bit for it in regular applications.
// TODO: control by a cargo feature
type RefCount = u16;

/// Epoch type determines the number of overwrites of components in storage.
// TODO: control by a cargo feature
type Epoch = u16;

type StorageId = u8;
static STORAGE_UID: AtomicUsize = AtomicUsize::new(0);

/// Pending reference counts updates.
#[derive(Debug)]
struct Pending {
    add_ref: Vec<Index>,
    sub_ref: Vec<Index>,
    epoch: Vec<Epoch>,
}

impl Pending {
    #[inline]
    fn drain_sub(&mut self) -> (Drain<Index>, &mut [Epoch]) {
        (self.sub_ref.drain(..), self.epoch.as_mut_slice())
    }

    #[inline]
    fn get_epoch(&self, index: usize) -> Epoch {
        *self.epoch.get(index).unwrap_or(&0)
    }
}

/// Shared pointer to the pending updates.
type PendingRef = Arc<Mutex<Pending>>;
