extern crate bit_field;
use self::bit_field::BitField;

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct PointerData(u64);

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
        self.0.get_bits(0..40) as usize
    }

    #[inline]
    pub fn get_epoch(&self) -> ::Epoch {
        self.0.get_bits(40..56) as ::Epoch
    }

    #[inline]
    pub fn get_storage_id(&self) -> ::StorageId {
        self.0.get_bits(56..64) as ::StorageId
    }

    #[inline]
    pub fn set_index(&mut self, value: usize) {
        self.0.set_bits(0..40, value as u64);
    }

    #[inline]
    pub fn set_epoch(&mut self, value: ::Epoch) {
        self.0.set_bits(40..56, value as u64);
    }

    #[inline]
    pub fn set_storage_id(&mut self, value: ::StorageId) {
        self.0.set_bits(56..64, value as u64);
    }
}
