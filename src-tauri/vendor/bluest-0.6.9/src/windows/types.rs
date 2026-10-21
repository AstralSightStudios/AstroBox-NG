use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use windows::core::HSTRING;
use windows::Foundation::Collections::{IIterable, IIterable_Impl, IIterator, IIterator_Impl};

#[windows::core::implement(IIterable<HSTRING>)]
pub(super) struct StringVec(Arc<Vec<HSTRING>>);

#[windows::core::implement(IIterator<HSTRING>)]
pub(super) struct StringIterator {
    vec: Arc<Vec<HSTRING>>,
    pos: AtomicUsize,
}

impl StringVec {
    pub fn new(strings: Vec<HSTRING>) -> Self {
        Self(Arc::new(strings))
    }
}

impl IIterable_Impl<HSTRING> for StringVec {
    fn First(&self) -> windows::core::Result<windows::Foundation::Collections::IIterator<HSTRING>> {
        Ok(StringIterator {
            vec: self.0.clone(),
            pos: AtomicUsize::new(0),
        }
        .into())
    }
}

impl IIterator_Impl<HSTRING> for StringIterator {
    fn Current(&self) -> windows::core::Result<HSTRING> {
        let pos = self.pos.load(Ordering::Relaxed);
        if pos < self.vec.len() {
            Ok(self.vec[pos].clone())
        } else {
            Err(windows::core::Error::OK)
        }
    }

    fn HasCurrent(&self) -> windows::core::Result<bool> {
        let pos = self.pos.load(Ordering::Relaxed);
        Ok(pos < self.vec.len())
    }

    fn MoveNext(&self) -> windows::core::Result<bool> {
        let pos = self.pos.fetch_add(1, Ordering::Relaxed);
        Ok(pos + 1 < self.vec.len())
    }

    fn GetMany(&self, items: &mut [<HSTRING as windows::core::Type<HSTRING>>::Default]) -> windows::core::Result<u32> {
        let pos = self.pos.fetch_add(items.len(), Ordering::Relaxed);
        if pos < self.vec.len() {
            let len = (self.vec.len() - pos).min(items.len());
            items[0..len].clone_from_slice(&self.vec[pos..][..len]);
            Ok(len as u32)
        } else {
            Ok(0)
        }
    }
}
