use nalgebra::Vector2;
use std::alloc::Layout;
use std::sync::atomic;
use std::{alloc, ptr};
use tokio::sync::Semaphore;

pub const CHUNK_SIZE: usize = 64;
const MAX_PERMITS: usize = usize::MAX >> 3;

pub struct Channel<U: Sized = f32> {
    inner: *mut U,
    memory_layout: alloc::Layout,
    size: Vector2<usize>,
    lock: tokio::sync::Semaphore,
}

pub struct WritePermit<'a> {
    lock: tokio::sync::SemaphorePermit<'a>,
}

pub struct ReadGuard<'a> {
    lock: tokio::sync::SemaphorePermit<'a>,
}

impl<U: Sized> Channel<U> {
    pub fn new(size: Vector2<usize>) -> Result<Self, crate::error::Error> {
        let layout = Layout::array(size.x * size.y)?;
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

    pub async fn write(&self) -> WritePermit {
        WritePermit {
            lock: self.lock.acquire_many(MAX_PERMITS as u32).await,
        }
    }

    pub async fn read(&self) -> ReadGuard {
        ReadGuard {
            lock: self.lock.acquire().await,
        }
    }
}

impl WritePermit {}
