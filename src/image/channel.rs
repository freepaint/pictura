use nalgebra::Vector2;
use std::alloc::Layout;
use std::ops::{Index, IndexMut};
use std::{alloc, ptr};
use tokio::sync::Semaphore;

pub const CHUNK_SIZE: usize = 64;
const MAX_PERMITS: usize = usize::MAX >> 3;

pub struct Channel<U: Sized = f32> {
    inner: *mut U,
    memory_layout: alloc::Layout,
    size: Vector2<u32>,
    lock: tokio::sync::Semaphore,
}

pub struct ReadGuard<'a, U: Sized = f32> {
    lock: tokio::sync::SemaphorePermit<'a>,
    channel: &'a Channel<U>,
}

pub struct WritePermit<'a, U: Sized = f32> {
    lock: tokio::sync::SemaphorePermit<'a>,
    channel: &'a Channel<U>,
}

pub struct WriteIter<'a, U: Sized = f32> {}

impl<U: Sized> Channel<U> {
    pub fn new(size: Vector2<u32>) -> Result<Self, crate::error::AllocError> {
        let layout = Layout::array::<U>((size.x * size.y) as usize)?;
        let data = unsafe { std::alloc::alloc_zeroed(layout) };
        if data.is_null() {
            return Err(crate::error::AllocError::Alloc.into());
        }
        Ok(Self {
            inner: data as *mut U,
            memory_layout: layout,
            size,
            lock: Semaphore::new(MAX_PERMITS),
        })
    }

    fn within(&self, pt: &Vector2<u32>) -> bool {
        0 <= pt.x && pt.x < self.size.x as u32 && 0 <= pt.y && pt.y < self.size.y as u32
    }

    unsafe fn offset(&self, pt: &Vector2<u32>) -> *mut U {
        self.inner
            .clone()
            .add((pt.y * self.size.x as u32 + pt.x) as usize)
    }

    unsafe fn read_unchecked(&self, pt: &Vector2<u32>) -> &U {
        self.offset(pt).as_ref().unwrap()
    }

    unsafe fn write_unchecked(&self, pt: &Vector2<u32>) -> &mut U {
        self.offset(pt).as_mut().unwrap()
    }

    pub async fn read(&self) -> ReadGuard<'_, U> {
        ReadGuard {
            lock: self.lock.acquire().await,
            channel: self,
        }
    }

    pub async fn write(&self) -> WritePermit<'_, U> {
        WritePermit {
            lock: self.lock.acquire_many(MAX_PERMITS as u32).await,
            channel: self,
        }
    }
}

impl<'a, U: Sized> ReadGuard<'a, U> {
    pub fn size(&self) -> Vector2<u32> {
        self.channel.size.map(|n| n as u32)
    }
}

impl<'a, U: Sized> WritePermit<'a, U> {
    pub fn size(&self) -> Vector2<u32> {
        self.channel.size.map(|n| n as u32)
    }

    pub fn iter_mut(&mut self) -> WriteIter {}
}

impl Iterator for WriteIter {
    fn next(&mut self) -> Option<Self::Item> {
        todo!()
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        todo!()
    }

    fn count(self) -> usize
    where
        Self: Sized,
    {
        self.size_hint().0
    }
}

impl<'a, U: Sized> Index<&Vector2<u32>> for ReadGuard<'a, U> {
    type Output = U;

    fn index(&self, index: &Vector2<u32>) -> &Self::Output {
        check_bounds(&index, &self.channel);
        unsafe { self.channel.read_unchecked(index) }
    }
}

impl<'a, U: Sized> Index<&Vector2<u32>> for WritePermit<'a, U> {
    type Output = U;

    fn index(&self, index: &Vector2<u32>) -> &Self::Output {
        check_bounds(&index, &self.channel);
        unsafe { self.channel.read_unchecked(index) }
    }
}

impl<'a, U: Sized> IndexMut<&Vector2<u32>> for WritePermit<'a, U> {
    fn index_mut(&mut self, index: &Vector2<u32>) -> &mut Self::Output {
        check_bounds(&index, &self.channel);
        unsafe { self.channel.write_unchecked(index) }
    }
}

#[inline(always)]
fn check_bounds<U: Sized>(pt: &Vector2<u32>, ch: &Channel<U>) {
    if !ch.within(pt) {
        panic!(
            "Unable to access {:?} with bounds (0, 0) -> {:?}",
            pt, &ch.size.x,
        );
    }
}
