use crate::error::Error::Alloc;
use std::alloc::LayoutErr;

pub enum Error {
    Alloc(AllocError),
}

pub enum AllocError {
    Alloc,
    Layout(LayoutErr),
}

impl From<AllocError> for Error {
    fn from(err: AllocError) -> Self {
        Self::Alloc(err)
    }
}

impl From<LayoutErr> for AllocError {
    fn from(err: LayoutErr) -> Self {
        Self::Layout(err)
    }
}
