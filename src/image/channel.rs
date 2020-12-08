use nalgebra::{Matrix2, Vector2};
use std::future::Future;
use std::ops::{Index, IndexMut};
use std::pin::Pin;
use std::sync::atomic;
use std::task::{Context, Poll};
use std::{alloc, ptr};

pub const CHUNK_SIZE: usize = 64;

pub struct Channel {
    inner: *mut f32,
    memory_layout: alloc::Layout,
    size: Vector2<usize>,
    ref_counter: ptr::NonNull<atomic::AtomicIsize>,
}

pub struct WriteGuard<'a> {
    channel: &'a mut Channel,
    ref_counter: ptr::NonNull<atomic::AtomicIsize>,
}

pub struct ReadGuard<'a> {
    channel: &'a Channel,
    ref_counter: ptr::NonNull<atomic::AtomicIsize>,
}

pub struct WriteIter<'a> {
    channel: &'a mut Channel,
    step: Vector2<usize>,
    ref_counter: ptr::NonNull<atomic::AtomicIsize>,
    done_flag: bool,
}

pub struct WriteIterGuard {
    /// Some boxes are null pointers, be careful
    inner: [*mut f32; CHUNK_SIZE],
    corners: Matrix2<usize>,
    size: Vector2<usize>,
    ref_counter: ptr::NonNull<atomic::AtomicIsize>,
}

pub struct WriteGuardAwaiter<'a>(Option<&'a mut Channel>);

pub struct ReadGuardAwaiter<'a>(Option<&'a Channel>);

impl Channel {
    pub fn new(width: usize, height: usize) -> Self {
        let layout = alloc::Layout::array::<f32>(width * height).unwrap();
        Self {
            inner: unsafe { alloc::alloc_zeroed(layout) as *mut f32 },
            size: Vector2::new(width, height),
            memory_layout: layout,
            ref_counter: Box::leak(Box::new(atomic::AtomicIsize::new(0))).into(),
        }
    }

    pub fn lock_write(&mut self) -> WriteGuard {
        // FIXME: This is a horrible solution, please fix this
        while self.locked() != 0 {
            std::thread::yield_now();
        }
        unsafe { self.ref_counter.as_ref() }.store(-1, atomic::Ordering::Release);
        let rc = self.ref_counter;
        WriteGuard {
            channel: self,
            ref_counter: rc,
        }
    }

    pub fn lock_read(&self) -> ReadGuard {
        // FIXME: This is a horrible solution, please fix this
        // Use condvar
        while self.locked() < 0 {
            std::thread::yield_now()
        }
        unsafe { self.force_lock_read() }
    }

    pub fn lock_write_async(&mut self) -> WriteGuardAwaiter {
        WriteGuardAwaiter(Some(self))
    }

    pub fn lock_read_async(&self) -> ReadGuardAwaiter {
        ReadGuardAwaiter(Some(self))
    }

    pub(self) unsafe fn force_lock_read(&self) -> ReadGuard {
        self.ref_counter
            .as_ref()
            .fetch_sub(1, atomic::Ordering::AcqRel);
        let rc = self.ref_counter;
        ReadGuard {
            channel: self,
            ref_counter: rc,
        }
    }

    #[inline]
    pub fn locked(&self) -> isize {
        unsafe { self.ref_counter.as_ref() }.load(atomic::Ordering::Acquire)
    }
}

impl Drop for Channel {
    fn drop(&mut self) {
        unsafe {
            alloc::dealloc(self.inner as *mut u8, self.memory_layout);
            Box::from_raw(self.ref_counter.as_ptr()); // get box from raw pointer and instantly drop it, deallocating it in the process
        }
    }
}

impl<'a> WriteGuard<'a> {
    pub fn write_raw(&mut self) -> &mut [f32] {
        unsafe {
            &mut *ptr::slice_from_raw_parts_mut(
                self.channel.inner,
                self.channel.size.x * self.channel.size.y,
            )
        }
    }

    pub fn chunked_iter_mut(&mut self) -> WriteIter {
        let rc = self.ref_counter;
        WriteIter {
            channel: self.channel,
            step: Vector2::default(),
            ref_counter: rc,
            done_flag: false,
        }
    }

    fn clone_from(&mut self, source: &ReadGuard) {
        if self.channel.size.x == source.channel.size.x
            && self.channel.size.y == source.channel.size.y
        {
            unsafe {
                ptr::copy_nonoverlapping(
                    source.channel.inner as *const u8,
                    self.channel.inner as *mut u8,
                    self.channel.memory_layout.size(),
                );
            }
        } else {
            unsafe {
                alloc::dealloc(self.channel.inner as *mut u8, self.channel.memory_layout);
                self.channel.inner =
                    std::alloc::alloc_zeroed(source.channel.memory_layout) as *mut f32;
            }
            self.channel.memory_layout = source.channel.memory_layout;
            self.channel.size.x = source.channel.size.x;
            self.channel.size.y = source.channel.size.y;
        }
    }
}

impl<'a> Drop for WriteGuard<'a> {
    fn drop(&mut self) {
        unsafe { self.ref_counter.as_ref() }.store(0, atomic::Ordering::Release);
    }
}

impl<'a> ReadGuard<'a> {
    pub fn read_raw(&self) -> &[f32] {
        unsafe {
            &*ptr::slice_from_raw_parts(
                self.channel.inner as *const f32,
                self.channel.size.x * self.channel.size.y,
            )
        }
    }

    fn clone(&self) -> Channel {
        let channel = Channel::new(self.channel.size.x, self.channel.size.y);
        unsafe {
            ptr::copy_nonoverlapping(
                self.channel.inner as *const u8,
                channel.inner as *mut u8,
                self.channel.memory_layout.size(),
            );
        }
        channel
    }
}

impl<'a> Drop for ReadGuard<'a> {
    fn drop(&mut self) {
        unsafe { self.ref_counter.as_ref() }.fetch_add(1, atomic::Ordering::AcqRel);
    }
}

impl<'a> Iterator for WriteIter<'a> {
    type Item = WriteIterGuard;

    fn next(&mut self) -> Option<Self::Item> {
        if self.done_flag {
            return None;
        }

        let base = self.channel.size.x * self.step.y + self.step.x;
        let goal = self.channel.size.y.min(self.step.y + CHUNK_SIZE) + self.channel.size.x;

        let mut chunks = [std::ptr::null_mut(); 64];

        for y in (base..goal).step_by(self.channel.size.x) {
            chunks[(y - base) / self.channel.size.x] = unsafe { self.channel.inner.add(y) };
        }

        let guard = WriteIterGuard {
            inner: chunks,
            corners: Matrix2::new(
                self.step.x,
                self.step.y,
                self.channel.size.x.min(self.step.x + CHUNK_SIZE),
                self.channel.size.y.min(self.step.y + CHUNK_SIZE),
            ),
            size: Vector2::new(
                self.channel.size.x.min(self.step.x + CHUNK_SIZE) - self.step.x,
                self.channel.size.y.min(self.step.y + CHUNK_SIZE) - self.step.y,
            ),
            ref_counter: self.ref_counter,
        };

        self.step.x += CHUNK_SIZE;
        if self.step.x >= self.channel.size.x {
            self.step.x = 0;
            self.step.y += 1;
            if self.step.y >= self.channel.size.y {
                self.done_flag = true;
            }
        }

        Some(guard)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.channel.size.y, Some(self.channel.size.y))
    }

    fn count(self) -> usize
    where
        Self: Sized,
    {
        self.channel.size.y
    }
}

impl WriteIterGuard {
    /// FIXME: Explain matrix
    pub fn offset(&self) -> Matrix2<u32> {
        self.corners.map(|n| n as u32)
    }

    pub fn bounds_check(&self, index: &Vector2<u32>) -> bool {
        index.x > self.size.x as u32 || index.y > self.size.y as u32
    }
}

impl Index<Vector2<u32>> for WriteIterGuard {
    type Output = f32;

    fn index(&self, index: Vector2<u32>) -> &Self::Output {
        if self.bounds_check(&index) {
            panic!("Index out of bounds");
        }
        unsafe {
            (self.inner[index.y as usize].add(index.x as usize))
                .as_ref()
                .unwrap()
        }
    }
}

impl IndexMut<Vector2<u32>> for WriteIterGuard {
    fn index_mut(&mut self, index: Vector2<u32>) -> &mut Self::Output {
        if self.bounds_check(&index) {
            panic!("Index out of bounds");
        }
        unsafe {
            (self.inner[index.y as usize].add(index.x as usize))
                .as_mut()
                .unwrap()
        }
    }
}

unsafe impl Send for WriteIterGuard {}

impl<'a> Future for WriteGuardAwaiter<'a> {
    type Output = WriteGuard<'a>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if self.0.is_none() {
            panic!("Future already completed");
        }
        let channel = self.0.as_ref().unwrap();
        if channel.locked() == 0 {
            unsafe { channel.ref_counter.as_ref() }.store(-1, atomic::Ordering::Release);
            let rc = channel.ref_counter;
            let channel = self.0.take().unwrap();
            Poll::Ready(WriteGuard {
                channel,
                ref_counter: rc,
            })
        } else {
            cx.waker().wake_by_ref();
            Poll::Pending
        }
    }
}

impl<'a> Future for ReadGuardAwaiter<'a> {
    type Output = ReadGuard<'a>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if self.0.is_none() {
            panic!("Future already completed");
        }
        let channel = self.0.as_ref().unwrap();
        if channel.locked() == 0 {
            unsafe { channel.ref_counter.as_ref() }.store(-1, atomic::Ordering::Release);
            let rc = channel.ref_counter;
            let channel = self.0.take().unwrap();
            Poll::Ready(ReadGuard {
                channel,
                ref_counter: rc,
            })
        } else {
            cx.waker().wake_by_ref();
            Poll::Pending
        }
    }
}
