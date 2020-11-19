use nalgebra::{Matrix2, Vector2};
use std::sync::atomic;
use std::{alloc, ptr};

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
    step: usize,
    ref_counter: ptr::NonNull<atomic::AtomicIsize>,
}

pub struct WriteIterGuard {
    inner: *mut [f32],
    offset: usize,
    size: Vector2<usize>,
    ref_counter: ptr::NonNull<atomic::AtomicIsize>,
}

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
        while self.locked() < 0 {
            std::thread::yield_now();
        }
        unsafe { self.ref_counter.as_ref() }.fetch_sub(1, atomic::Ordering::AcqRel);
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
        unsafe { alloc::dealloc(self.inner as *mut u8, self.memory_layout) }
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
            step: 0,
            ref_counter: rc,
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
        if self.step >= self.channel.size.y {
            None
        } else {
            let offset = self.channel.size.y * self.step;
            self.step += 1;
            Some(WriteIterGuard {
                inner: ptr::slice_from_raw_parts_mut(
                    unsafe { self.channel.inner.add(offset) },
                    self.channel.size.x,
                ),
                offset,
                size: Vector2::from_data(self.channel.size.data),
                ref_counter: self.ref_counter,
            })
        }
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

unsafe impl Send for WriteIterGuard {}

impl WriteIterGuard {
    /// FIXME: Explain matrix
    pub fn offset(&self) -> Matrix2<u32> {
        // lu = left upper
        // rl = right lower
        let lux = (self.offset % self.size.x) as u32;
        let luy = (self.offset / self.size.x) as u32;
        let rlx = (self.offset % self.size.x + self.offset) as u32;
        let rly = (self.offset / self.size.x) as u32;
        Matrix2::new(lux, luy, rlx, rly)
    }

    pub fn get(&self) -> &[f32] {
        unsafe { &*self.inner }
    }

    pub fn get_mut(&mut self) -> &mut [f32] {
        unsafe { &mut *self.inner }
    }
}
