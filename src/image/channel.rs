use std::sync::atomic;
use std::{alloc, ptr};

pub struct Channel {
    inner: *mut f32,
    layout: alloc::Layout,
    width: usize,
    height: usize,
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
    width: usize,
    height: usize,
    ref_counter: ptr::NonNull<atomic::AtomicIsize>,
}

impl Channel {
    pub fn new(width: usize, height: usize) -> Self {
        let layout = alloc::Layout::array::<f32>(width * height).unwrap();
        Self {
            inner: unsafe { alloc::alloc_zeroed(layout) as *mut f32 },
            width,
            height,
            layout,
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
        unsafe { alloc::dealloc(self.inner as *mut u8, self.layout) }
    }
}

impl<'a> WriteGuard<'a> {
    pub fn write_raw(&mut self) -> &mut [f32] {
        unsafe {
            &mut *ptr::slice_from_raw_parts_mut(
                self.channel.inner,
                self.channel.width * self.channel.height,
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
        if self.channel.width == source.channel.width
            && self.channel.height == source.channel.height
        {
            unsafe {
                ptr::copy_nonoverlapping(
                    source.channel.inner as *const u8,
                    self.channel.inner as *mut u8,
                    self.channel.layout.size(),
                );
            }
        } else {
            unsafe {
                alloc::dealloc(self.channel.inner as *mut u8, self.channel.layout);
                self.channel.inner = std::alloc::alloc_zeroed(source.channel.layout) as *mut f32;
            }
            self.channel.layout = source.channel.layout;
            self.channel.width = source.channel.width;
            self.channel.height = source.channel.height;
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
                self.channel.width * self.channel.height,
            )
        }
    }

    fn clone(&self) -> Channel {
        let channel = Channel::new(self.channel.width, self.channel.height);
        unsafe {
            ptr::copy_nonoverlapping(
                self.channel.inner as *const u8,
                channel.inner as *mut u8,
                self.channel.layout.size(),
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
        if self.step >= self.channel.height {
            None
        } else {
            let offset = self.channel.height * self.step;
            self.step += 1;
            Some(WriteIterGuard {
                inner: ptr::slice_from_raw_parts_mut(
                    unsafe { self.channel.inner.add(offset) },
                    self.channel.width,
                ),
                offset,
                width: self.channel.width,
                height: self.channel.height,
                ref_counter: self.ref_counter.clone(),
            })
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.channel.height, Some(self.channel.height))
    }

    fn count(self) -> usize
    where
        Self: Sized,
    {
        self.channel.height
    }
}

unsafe impl Send for WriteIterGuard {}

impl WriteIterGuard {
    // Fixme: This is horrible
    pub fn offsets(&self) -> ((u32, u32), (u32, u32)) {
        (
            (
                (self.offset % self.width) as u32,
                (self.offset / self.width) as u32,
            ),
            (
                (self.offset % self.width + self.offset) as u32,
                (self.offset / self.width) as u32,
            ),
        )
    }

    pub fn get(&self) -> &[f32] {
        unsafe { &*self.inner }
    }

    pub fn get_mut(&mut self) -> &mut [f32] {
        unsafe { &mut *self.inner }
    }
}
