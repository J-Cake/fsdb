use std::cmp::Ordering;

#[inline]
pub fn round(x: u64, n: u64) -> u64 {
    x + (n - x % n)
}

#[derive(Copy, Clone, Debug)]
pub struct Array {
    pub length: u64,
    pub offset: u64,
}

impl Array {
    pub fn to_range(self) -> std::ops::Range<usize> {
        self.offset as usize..(self.offset + self.length) as usize
    }

    pub fn end(&self) -> u64 {
        self.offset + self.length
    }
}

impl PartialEq for Array {
    fn eq(&self, other: &Self) -> bool {
        self.offset == other.offset
    }
}

impl Eq for Array {}

impl PartialOrd for Array {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(Ord::cmp(self, other))
    }
}

impl Ord for Array {
    fn cmp(&self, other: &Self) -> Ordering {
        self.offset.cmp(&other.offset)
    }
}
