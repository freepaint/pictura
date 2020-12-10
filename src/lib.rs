pub mod error;
#[allow(dead_code)]
pub mod image;

#[cfg(test)]
mod tests {
    use mockalloc::Mockalloc;
    use std::alloc::System;

    #[global_allocator]
    static ALLOCATOR: Mockalloc<System> = Mockalloc(System);
}
