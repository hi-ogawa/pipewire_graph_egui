use std::{convert::TryFrom, fmt::Debug};

#[derive(Debug)]
pub enum DataType {
    Invalid,
    /// Pointer to memory, the data field in struct [`Data`] is set.
    MemPtr,
    /// Generic fd, `mmap` to get to memory
    MemFd,
    /// Fd to `dmabuf` memory
    DmaBuf,
    /// Memory is identified with an id
    MemId,
    Other(u32),
}

impl DataType {
    pub fn from_raw(raw: u32) -> DataType {
        match raw {
            spa_sys::SPA_DATA_Invalid => Self::Invalid,
            spa_sys::SPA_DATA_MemPtr => Self::MemPtr,
            spa_sys::SPA_DATA_MemFd => Self::MemFd,
            spa_sys::SPA_DATA_DmaBuf => Self::DmaBuf,
            spa_sys::SPA_DATA_MemId => Self::MemId,
            other => Self::Other(other),
        }
    }

    pub fn as_raw(&self) -> u32 {
        match self {
            Self::Invalid => spa_sys::SPA_DATA_Invalid,
            Self::MemPtr => spa_sys::SPA_DATA_MemPtr,
            Self::MemFd => spa_sys::SPA_DATA_MemFd,
            Self::DmaBuf => spa_sys::SPA_DATA_DmaBuf,
            Self::MemId => spa_sys::SPA_DATA_MemId,
            Self::Other(other) => *other,
        }
    }
}

bitflags::bitflags! {
    pub struct DataFlags: u32 {
        /// Data is readable
        const READABLE = 1<<0;
        /// Data is writable
        const WRITABLE = 1<<1;
        /// Data pointer can be changed
        const DYNAMIC = 1<<2;
        const READWRITE = Self::READABLE.bits | Self::WRITABLE.bits;
    }
}

#[repr(transparent)]
pub struct Data(spa_sys::spa_data);

impl Data {
    pub fn as_raw(&self) -> &spa_sys::spa_data {
        &self.0
    }

    pub fn type_(&self) -> DataType {
        DataType::from_raw(self.0.type_)
    }

    pub fn flags(&self) -> DataFlags {
        DataFlags::from_bits_truncate(self.0.flags)
    }

    // FIXME: Add bindings for the fd field, but how to detect when it is not set / invalid?

    pub fn data(&mut self) -> Option<&mut [u8]> {
        // FIXME: For safety, perhaps only return a non-mut slice when DataFlags::WRITABLE is not set?
        if self.0.data.is_null() {
            None
        } else {
            unsafe {
                Some(std::slice::from_raw_parts_mut(
                    self.0.data as *mut u8,
                    usize::try_from(self.0.maxsize).unwrap(),
                ))
            }
        }
    }

    pub fn chunk(&self) -> &Chunk {
        assert_ne!(self.0.chunk, std::ptr::null_mut());
        unsafe {
            let chunk: *const spa_sys::spa_chunk = self.0.chunk;
            &*(chunk as *const Chunk)
        }
    }

    pub fn chunk_mut(&mut self) -> &mut Chunk {
        assert_ne!(self.0.chunk, std::ptr::null_mut());
        unsafe {
            let chunk: *mut spa_sys::spa_chunk = self.0.chunk;
            &mut *(chunk as *mut Chunk)
        }
    }
}

impl Debug for Data {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Data")
            .field("type", &self.type_())
            .field("flags", &self.flags())
            // FIXME: Add fd
            .field("data", &self.0.data) // Only print the pointer here, as we don't want to print a (potentially very big) slice.
            .field("chunk", &self.chunk())
            .finish()
    }
}

bitflags::bitflags! {
    pub struct ChunkFlags: i32 {
        /// Chunk data is corrupted in some way
        const CORRUPTED = 1<<0;
    }
}

#[repr(transparent)]
pub struct Chunk(spa_sys::spa_chunk);

impl Chunk {
    pub fn as_raw(&self) -> &spa_sys::spa_chunk {
        &self.0
    }

    pub fn size(&self) -> u32 {
        self.0.size
    }

    pub fn size_mut(&mut self) -> &mut u32 {
        &mut self.0.size
    }

    pub fn offset(&self) -> u32 {
        self.0.offset
    }

    pub fn offset_mut(&mut self) -> &mut u32 {
        &mut self.0.offset
    }

    pub fn stride(&self) -> i32 {
        self.0.stride
    }

    pub fn stride_mut(&mut self) -> &mut i32 {
        &mut self.0.stride
    }

    pub fn flags(&self) -> ChunkFlags {
        ChunkFlags::from_bits_truncate(self.0.flags)
    }
}

impl Debug for Chunk {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Chunk")
            .field("offset", &self.offset())
            .field("size", &self.size())
            .field("stride", &self.stride())
            .field("flags", &self.flags())
            .finish()
    }
}
