use crate::{Epoch, Index, StorageId};

#[derive(Copy, Clone, Debug, PartialEq, Hash)]
pub struct PointerData(u64);

#[cfg(target_pointer_width = "32")]
const INDEX_BITS: u8 = 20;
#[cfg(target_pointer_width = "32")]
const EPOCH_BITS: u8 = 8;
#[cfg(target_pointer_width = "32")]
const STORAGE_ID_BITS: u8 = 4;

#[cfg(target_pointer_width = "64")]
const INDEX_BITS: u8 = 40;
#[cfg(target_pointer_width = "64")]
const EPOCH_BITS: u8 = 16;
#[cfg(target_pointer_width = "64")]
const STORAGE_ID_BITS: u8 = 8;

const INDEX_MASK: u64 = (1 << INDEX_BITS) - 1;
const EPOCH_OFFSET: u8 = INDEX_BITS;
const EPOCH_MASK: u64 = ((1 << EPOCH_BITS) - 1) << EPOCH_OFFSET;
const STORAGE_ID_OFFSET: u8 = EPOCH_OFFSET + EPOCH_BITS;
const STORAGE_ID_MASK: u64 = ((1 << STORAGE_ID_BITS) - 1) << STORAGE_ID_OFFSET;

impl PointerData {
    #[inline]
    pub fn new(index: Index, epoch: Epoch, storage: StorageId) -> Self {
        debug_assert_eq!(index >> INDEX_BITS, 0);
        PointerData(
            index as u64
                + ((u64::from(epoch)) << EPOCH_OFFSET)
                + ((u64::from(storage)) << STORAGE_ID_OFFSET),
        )
    }

    #[inline]
    pub fn get_index(self) -> Index {
        (self.0 & INDEX_MASK) as Index
    }

    #[inline]
    pub fn get_epoch(self) -> Epoch {
        ((self.0 & EPOCH_MASK) >> EPOCH_OFFSET) as Epoch
    }

    #[inline]
    pub fn get_storage_id(self) -> StorageId {
        ((self.0 & STORAGE_ID_MASK) >> STORAGE_ID_OFFSET) as StorageId
    }

    #[inline]
    pub fn with_epoch(self, epoch: Epoch) -> PointerData {
        PointerData((self.0 & !EPOCH_MASK) + ((u64::from(epoch)) << EPOCH_OFFSET))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem::size_of;

    #[test]
    fn sizes() {
        #[cfg(target_pointer_width = "32")]
        assert_eq!(INDEX_BITS + EPOCH_BITS + STORAGE_ID_BITS, 32);
        #[cfg(target_pointer_width = "64")]
        assert_eq!(INDEX_BITS + EPOCH_BITS + STORAGE_ID_BITS, 64);
        assert!(size_of::<Index>() * 8 >= INDEX_BITS as usize);
        assert!(size_of::<Epoch>() * 8 >= EPOCH_BITS as usize);
        assert!(size_of::<StorageId>() * 8 >= STORAGE_ID_BITS as usize);
    }

    #[test]
    fn new() {
        let pd = PointerData::new(1, 2, 3);
        assert_eq!(pd.get_index(), 1);
        assert_eq!(pd.get_epoch(), 2);
        assert_eq!(pd.get_storage_id(), 3);
        assert_eq!(pd.with_epoch(5).get_epoch(), 5);
    }
}
