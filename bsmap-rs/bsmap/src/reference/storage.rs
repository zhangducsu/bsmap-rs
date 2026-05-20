//! 参考序列存储后端抽象。
//!
//! 提供 `BinSeqStorage` trait，支持 `Vec<u64>` 堆内存和 `memmap2::Mmap` 文件映射两种后端。

use std::fmt;

/// 参考序列存储后端抽象。
pub trait BinSeqStorage: Send + Sync + fmt::Debug {
    /// 以 u64 slice 形式访问存储的序列数据。
    fn as_slice(&self) -> &[u64];

    /// 获取存储的 u64 word 数量。
    fn len(&self) -> usize;

    /// 是否为空。
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// 堆内存存储后端（现有行为，全量驻留内存）。
#[derive(Debug, Clone)]
pub struct VecStorage {
    data: Vec<u64>,
}

impl VecStorage {
    pub fn new(data: Vec<u64>) -> Self {
        Self { data }
    }
}

impl BinSeqStorage for VecStorage {
    #[inline]
    fn as_slice(&self) -> &[u64] {
        &self.data
    }

    #[inline]
    fn len(&self) -> usize {
        self.data.len()
    }
}

impl From<Vec<u64>> for VecStorage {
    fn from(data: Vec<u64>) -> Self {
        Self::new(data)
    }
}

/// mmap 文件映射存储后端（支持偏移量，用于 .bsi 文件中的子区域映射）。
#[derive(Debug)]
pub struct MmapStorage {
    mmap: memmap2::Mmap,
    offset: usize,
    len: usize,
}

impl MmapStorage {
    /// 从 memmap2::Mmap 创建，指定字节偏移和 u64 word 数量。
    pub fn with_offset(mmap: memmap2::Mmap, offset: usize, len: usize) -> Self {
        assert!(offset % 8 == 0, "offset must be 8-byte aligned, got {}", offset);
        assert!(
            mmap.len() >= offset + len * 8,
            "mmap region out of bounds: mmap len={}, offset={}, words={}",
            mmap.len(),
            offset,
            len
        );
        Self { mmap, offset, len }
    }

    /// 从 memmap2::Mmap 创建（offset=0 的便捷方法）。
    pub fn new(mmap: memmap2::Mmap, len: usize) -> Self {
        Self::with_offset(mmap, 0, len)
    }
}

impl BinSeqStorage for MmapStorage {
    #[inline]
    fn as_slice(&self) -> &[u64] {
        unsafe {
            std::slice::from_raw_parts(
                (self.mmap.as_ptr() as *const u8).add(self.offset) as *const u64,
                self.len,
            )
        }
    }

    #[inline]
    fn len(&self) -> usize {
        self.len
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vec_storage_basic() {
        let data = vec![1u64, 2, 3, 4, 5];
        let storage = VecStorage::new(data.clone());
        assert_eq!(storage.len(), 5);
        assert_eq!(storage.as_slice(), &data[..]);
        assert!(!storage.is_empty());
    }

    #[test]
    fn test_vec_storage_empty() {
        let storage = VecStorage::new(vec![]);
        assert!(storage.is_empty());
        assert_eq!(storage.as_slice(), &[]);
    }

    #[test]
    fn test_mmap_storage() {
        use std::io::Write;

        let mut tmp = tempfile::NamedTempFile::new().unwrap();
        let data: Vec<u64> = vec![10, 20, 30, 40, 50];
        for &val in &data {
            tmp.write_all(&val.to_le_bytes()).unwrap();
        }
        tmp.flush().unwrap();

        let file = std::fs::File::open(tmp.path()).unwrap();
        let mmap = unsafe { memmap2::Mmap::map(&file).unwrap() };
        let storage = MmapStorage::new(mmap, data.len());

        assert_eq!(storage.len(), 5);
        assert_eq!(storage.as_slice(), &data[..]);
    }
}
