use nalgebra::Vector2;
use std::sync::atomic;
use std::{alloc, ptr};

pub const CHUNK_SIZE: usize = 64;

pub struct Channel {
    inner: *mut f32,
    memory_layout: alloc::Layout,
    size: Vector2<usize>,
    ref_counter: ptr::NonNull<atomic::AtomicIsize>,
}
