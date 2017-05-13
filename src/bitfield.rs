#[derive(Copy, Clone, Debug, PartialEq)]
pub struct PointerData(u64);

const INDEX_MASK: u64 = 0x0000_00FF_FFFF_FFFF;
const EPOCH_MASK: u64 = 0x00FF_FF00_0000_0000;
const SID_MASK: u64 = 0xFF00_0000_0000_0000;

impl PointerData {
    #[inline]
    pub fn new(index: usize, epoch: ::Epoch, storage: ::StorageId) -> Self {
        let mut p = PointerData(0);
        p.set_index(index);
        p.set_epoch(epoch);
        p.set_storage_id(storage);
        p
    }

    #[inline]
    pub fn get_index(&self) -> usize {
        (self.0 & INDEX_MASK) as usize
    }

    #[inline]
    pub fn get_epoch(&self) -> ::Epoch {
        ((self.0 & EPOCH_MASK)>>40) as ::Epoch
    }

    #[inline]
    pub fn get_storage_id(&self) -> ::StorageId {
        ((self.0 & SID_MASK)>>56) as ::StorageId
    }

    #[inline]
    pub fn set_index(&mut self, value: usize) {
        debug_assert!(value as u64 <= INDEX_MASK);
        self.0 = (self.0 & (!INDEX_MASK)) + value as u64;
    }

    #[inline]
    pub fn set_epoch(&mut self, value: ::Epoch) {
        self.0 = (self.0 & (!EPOCH_MASK)) + ((value as u64) << 40);
    }

    #[inline]
    pub fn set_storage_id(&mut self, value: ::StorageId) {
        self.0 = (self.0 & (!SID_MASK)) + ((value as u64) << 56);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rw_pointer_data() {
        let mut pd = PointerData::new(1, 2, 3);
        assert_eq!(pd.get_index(), 1);
        assert_eq!(pd.get_epoch(), 2);
        assert_eq!(pd.get_storage_id(), 3);
        pd.set_index(2);
        assert_eq!(pd.get_index(), 2);
        pd.set_epoch(4);
        assert_eq!(pd.get_epoch(), 4);
        pd.set_storage_id(6);
        assert_eq!(pd.get_storage_id(), 6);
    }
}
