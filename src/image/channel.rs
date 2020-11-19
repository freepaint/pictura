use std::ops::Deref;
use std::{alloc, ptr, sync};

pub struct Channel {
    inner: *mut f32,
    layout: alloc::Layout,
    width: usize,
    height: usize,
    ref_counter: sync::Arc<()>,
}

pub struct WriteIter<'a> {
    channel: &'a mut Channel,
    step: usize,
    ref_counter: sync::Arc<()>,
}

pub struct WriteGuard {
    inner: *mut [f32],
    offset: usize,
    width: usize,
    height: usize,
    ref_counter: sync::Arc<()>,
}

impl Channel {
    pub fn new(width: usize, height: usize) -> Self {
        let layout = alloc::Layout::array::<f32>(width * height).unwrap();
        Self {
            inner: unsafe { alloc::alloc_zeroed(layout) as *mut f32 },
            width,
            height,
            layout,
            ref_counter: sync::Arc::new(()),
        }
    }

    pub fn read_raw(&self) -> &[f32] {
        unsafe { &*ptr::slice_from_raw_parts(self.inner as *const f32, self.width * self.height) }
    }

    pub fn write_raw(&mut self) -> &mut [f32] {
        unsafe { &mut *ptr::slice_from_raw_parts_mut(self.inner, self.width * self.height) }
    }

    pub fn chunked_iter_mut(&mut self) -> WriteIter {
        let rc = self.ref_counter.clone();
        WriteIter {
            channel: self,
            step: 0,
            ref_counter: rc,
        }
    }

    #[inline]
    pub fn locked(&self) -> bool {
        sync::Arc::strong_count(&self.ref_counter) != 1
    }
}

impl Clone for Channel {
    fn clone(&self) -> Self {
        let layout = self.layout;
        let inner = unsafe { alloc::alloc_zeroed(layout) as *mut f32 };
        unsafe {
            ptr::copy_nonoverlapping(
                self.inner as *const u8,
                inner as *mut u8,
                self.layout.size(),
            );
        }
        Self {
            inner,
            width: self.width,
            height: self.height,
            layout,
            ref_counter: sync::Arc::new(()),
        }
    }

    fn clone_from(&mut self, source: &Self) {
        if self.width == source.width && self.height == source.height {
            unsafe {
                ptr::copy_nonoverlapping(
                    source.inner as *const u8,
                    self.inner as *mut u8,
                    self.layout.size(),
                );
            }
        } else {
            unsafe {
                alloc::dealloc(self.inner as *mut u8, self.layout);
                self.inner = std::alloc::alloc_zeroed(source.layout) as *mut f32;
            }
            self.layout = source.layout;
            self.width = source.width;
            self.height = source.height;
        }
    }
}

impl Drop for Channel {
    fn drop(&mut self) {
        unsafe { alloc::dealloc(self.inner as *mut u8, self.layout) }
    }
}

impl<'a> Iterator for WriteIter<'a> {
    type Item = WriteGuard;

    fn next(&mut self) -> Option<Self::Item> {
        if self.step >= self.channel.height {
            None
        } else {
            let offset = self.channel.height * self.step;
            self.step += 1;
            Some(WriteGuard {
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

impl WriteGuard {
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
