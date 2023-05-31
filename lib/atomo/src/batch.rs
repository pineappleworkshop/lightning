use fxhash::FxHashMap;

pub type BoxedVec = Box<[u8]>;

pub type BatchHashMap = FxHashMap<BoxedVec, Operation>;

/// A vertical batch contains a list of slots for each different table. Putting
/// the [`VerticalBatch`] into a [`SnapshotList`] will provide a valid snapshot
/// list.
pub struct VerticalBatch(Vec<BatchHashMap>);

/// The change on a value.
pub enum Operation {
    Remove,
    Insert(BoxedVec),
}

impl VerticalBatch {
    /// Returns a new empty vertical batch with the given size. The
    /// size can be used for the number of tables.
    #[inline(always)]
    pub fn new(size: usize) -> Self {
        let mut vec = Vec::with_capacity(size);
        vec.resize_with(size, || FxHashMap::default());
        VerticalBatch(vec)
    }

    /// Consume the vertical batch and returns the underlying vector of batches.
    #[inline(always)]
    pub fn into_raw(self) -> Vec<BatchHashMap> {
        self.0
    }

    #[inline(always)]
    pub fn get(&self, index: usize) -> &BatchHashMap {
        debug_assert!(index < self.0.len());
        &self.0[index]
    }

    /// Return a reference to a single slot in the vertical batch.
    ///
    /// # Safety
    ///
    /// It is up to the caller to ensure:
    ///
    /// 1. The index is only claimed once.
    /// 2. The reference's lifetime is bounded to this [`VerticalBatch`].
    #[inline(always)]
    pub unsafe fn claim(&self, index: usize) -> BatchReference {
        let x = self.0.get_unchecked(index) as *const BatchHashMap as *mut BatchHashMap;
        BatchReference(x)
    }
}

/// The reference to a single batch slot.
pub struct BatchReference(*mut BatchHashMap);

impl BatchReference {
    #[inline(always)]
    pub fn as_mut(&self) -> &mut BatchHashMap {
        unsafe { &mut *self.0 }
    }
}
