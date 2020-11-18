use std::{alloc, ptr};

pub struct Channel {
    inner: *mut f32,
    layout: alloc::Layout,
    width: u32,
    height: u32,
}

pub struct WriteIter<'a> {
    channel: &'a mut Channel,
    step: u32,
}

impl Channel {
    pub fn new(width: u32, height: u32) -> Self {
        let layout = alloc::Layout::array::<f32>((width * height) as usize).unwrap();
        Self {
            inner: unsafe { alloc::alloc_zeroed(layout) as *mut f32 },
            width,
            height,
            layout,
        }
    }

    pub fn read_raw(&self) -> &[f32] {
        unsafe {
            &*ptr::slice_from_raw_parts(
                self.inner as *const f32,
                (self.width * self.height) as usize,
            )
        }
    }

    pub fn write_raw(&mut self) -> &mut [f32] {
        unsafe {
            &mut *ptr::slice_from_raw_parts_mut(self.inner, (self.width * self.height) as usize)
        }
    }

    pub fn chunked_iter_mut(&mut self) -> WriteIter {
        WriteIter {
            channel: self,
            step: 0,
        }
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
    type Item = *mut [f32];

    fn next(&mut self) -> Option<Self::Item> {
        if self.step >= self.channel.height {
            None
        } else {
            self.step += 1;
            Some(ptr::slice_from_raw_parts_mut(
                unsafe {
                    self.channel
                        .inner
                        .add((self.channel.height * (self.step - 1)) as usize)
                },
                self.channel.width as usize,
            ))
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (
            self.channel.height as usize,
            Some(self.channel.height as usize),
        )
    }

    fn count(self) -> usize
    where
        Self: Sized,
    {
        self.channel.height as usize
    }
}
