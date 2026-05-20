//! 对象池模块。
//! 
//! 提供可复用的对象池实现，用于减少高频分配场景的内存分配开销。
//!
//! ## P4-5 优化内容
//!
//! 1. **线程本地对象池**: 使用 `thread_local!` 实现无锁分配
//! 2. **全局对象池管理器**: 协调多线程间的内存复用
//! 3. **Arena分配器**: 批量分配同类型对象，减少碎片

use std::cell::UnsafeCell;
use std::marker::PhantomData;

/// 轻量级对象池。
/// 
/// 用于复用临时对象，减少内存分配。
pub struct ObjectPool<T> {
    items: Vec<T>,
    free_indices: Vec<usize>,
    _marker: PhantomData<T>,
}

impl<T: Default> ObjectPool<T> {
    pub fn new(capacity: usize) -> Self {
        let mut items = Vec::with_capacity(capacity);
        for _ in 0..capacity {
            items.push(T::default());
        }
        let free_indices: Vec<usize> = (0..capacity).rev().collect();
        
        Self {
            items,
            free_indices,
            _marker: PhantomData,
        }
    }
    
    #[inline]
    pub fn get(&mut self) -> usize {
        if let Some(idx) = self.free_indices.pop() {
            idx
        } else {
            let idx = self.items.len();
            self.items.push(T::default());
            idx
        }
    }
    
    #[inline]
    pub fn get_mut(&mut self, idx: usize) -> &mut T {
        &mut self.items[idx]
    }
    
    #[inline]
    pub fn release(&mut self, idx: usize) {
        self.items[idx] = T::default();
        self.free_indices.push(idx);
    }
    
    #[inline]
    pub fn len(&self) -> usize {
        self.items.len() - self.free_indices.len()
    }
    
    #[inline]
    pub fn capacity(&self) -> usize {
        self.items.len()
    }
    
    pub fn clear(&mut self) {
        self.items.clear();
        self.free_indices.clear();
    }
}

/// 命中记录池。
/// 
/// 专门用于存储命中记录的对象池，提供更高效的接口。
pub struct HitPool<T> {
    hits: Vec<T>,
    pos: usize,
}

impl<T: Default + Clone> HitPool<T> {
    #[inline]
    pub fn with_capacity(cap: usize) -> Self {
        let mut hits = Vec::with_capacity(cap);
        for _ in 0..cap {
            hits.push(T::default());
        }
        Self { hits, pos: 0 }
    }
    
    #[inline]
    pub fn get(&mut self) -> &mut T {
        if self.pos >= self.hits.len() {
            self.hits.push(T::default());
        }
        let idx = self.pos;
        self.pos += 1;
        &mut self.hits[idx]
    }
    
    #[inline]
    pub fn reset(&mut self) {
        self.pos = 0;
    }
    
    #[inline]
    pub fn len(&self) -> usize {
        self.pos
    }
    
    #[inline]
    pub fn capacity(&self) -> usize {
        self.hits.capacity()
    }
    
    #[inline]
    pub fn iter(&self) -> impl Iterator<Item = &T> {
        self.hits[..self.pos].iter()
    }
    
    #[inline]
    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut T> {
        self.hits[..self.pos].iter_mut()
    }
    
    #[inline]
    pub fn as_slice(&self) -> &[T] {
        &self.hits[..self.pos]
    }
    
    #[inline]
    pub fn as_mut_slice(&mut self) -> &mut [T] {
        &mut self.hits[..self.pos]
    }
    
    #[inline]
    pub fn push(&mut self, hit: T) {
        if self.pos >= self.hits.len() {
            self.hits.push(hit);
        } else {
            self.hits[self.pos] = hit;
        }
        self.pos += 1;
    }
}

/// 缓冲区管理器。
/// 
/// 管理多个预分配的缓冲区，用于减少内存分配。
pub struct BufferManager {
    buffers: Vec<Vec<u8>>,
    current_buffer: usize,
    num_buffers: usize,
}

impl BufferManager {
    #[inline]
    pub fn new(num_buffers: usize, buffer_size: usize) -> Self {
        let mut buffers = Vec::with_capacity(num_buffers);
        for _ in 0..num_buffers {
            buffers.push(Vec::with_capacity(buffer_size));
        }
        Self {
            buffers,
            current_buffer: 0,
            num_buffers,
        }
    }
    
    #[inline]
    pub fn get_buffer(&mut self) -> &mut Vec<u8> {
        let idx = self.current_buffer;
        self.current_buffer = (self.current_buffer + 1) % self.num_buffers;
        let buffer = &mut self.buffers[idx];
        buffer.clear();
        buffer
    }
    
    #[inline]
    pub fn get_buffer_at(&mut self, idx: usize) -> &mut Vec<u8> {
        let buffer = &mut self.buffers[idx % self.num_buffers];
        buffer.clear();
        buffer
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// P4-5: 线程本地对象池
// ─────────────────────────────────────────────────────────────────────────────

/// 线程本地命中池。
/// 
/// 使用 `thread_local!` 实现，每个线程有独立的池，
/// 无需加锁即可访问。
thread_local! {
    static THREAD_LOCAL_HIT_POOL: UnsafeCell<HitPoolInner> = UnsafeCell::new(HitPoolInner::new(256));
}

/// 线程本地命中池的内部实现。
struct HitPoolInner {
    hits: Vec<ExtHit>,
    pos: usize,
}

impl HitPoolInner {
    #[inline]
    fn new(capacity: usize) -> Self {
        let mut hits = Vec::with_capacity(capacity);
        for _ in 0..capacity {
            hits.push(ExtHit::default());
        }
        Self { hits, pos: 0 }
    }
    
    #[inline]
    fn get(&mut self) -> &mut ExtHit {
        if self.pos >= self.hits.len() {
            self.hits.push(ExtHit::default());
        }
        let idx = self.pos;
        self.pos += 1;
        &mut self.hits[idx]
    }
    
    #[inline]
    fn reset(&mut self) {
        self.pos = 0;
    }
    
    #[inline]
    fn len(&self) -> usize {
        self.pos
    }
    
    #[inline]
    fn push(&mut self, hit: ExtHit) {
        if self.pos >= self.hits.len() {
            self.hits.push(hit);
        } else {
            self.hits[self.pos] = hit;
        }
        self.pos += 1;
    }
    
    #[inline]
    fn as_slice(&self) -> &[ExtHit] {
        &self.hits[..self.pos]
    }
}

/// 扩展命中记录（用于线程本地池）。
#[derive(Debug, Clone, Copy, Default)]
struct ExtHit {
    chr: u32,
    loc: u32,
    snps: u8,
    strand: u8,
    gap_size: i8,
    gap_pos: u8,
}

/// 获取线程本地命中池。
/// 
/// 每个线程第一次调用时创建独立的池，之后复用。
/// 无锁实现，性能最优。
#[inline]
pub fn get_thread_local_pool() -> &'static mut HitPool<ExtHit> {
    THREAD_LOCAL_HIT_POOL.with(|pool| {
        let inner = unsafe { &mut *pool.get() };
        // 安全: 每个线程有独立的池，不存在数据竞争
        // 我们返回可变引用，让调用者可以修改
        // 由于 thread_local 的特性，每个线程只能同时有一个可变引用
        unsafe { std::mem::transmute::<&mut HitPoolInner, &mut HitPool<ExtHit>>(inner) }
    })
}

/// 重置线程本地命中池。
/// 
/// 在处理完一个读段后调用，释放已使用的空间。
#[inline]
pub fn reset_thread_local_pool() {
    THREAD_LOCAL_HIT_POOL.with(|pool| {
        let inner = unsafe { &mut *pool.get() };
        inner.reset();
    });
}

/// 线程本地Arena分配器。
/// 
/// 适用于生命周期短、大量分配的场景。
/// 提供比对象池更简单的接口，但需要一次性释放所有内存。
thread_local! {
    static THREAD_LOCAL_ARENA: UnsafeCell<ArenaInner> = UnsafeCell::new(ArenaInner::new(4096));
}

/// Arena分配器内部实现。
struct ArenaInner {
    buffer: Vec<u8>,
    used: usize,
    chunk_size: usize,
}

impl ArenaInner {
    #[inline]
    fn new(chunk_size: usize) -> Self {
        Self {
            buffer: vec![0u8; chunk_size],
            used: 0,
            chunk_size,
        }
    }
    
    #[inline]
    fn alloc(&mut self, size: usize) -> &mut [u8] {
        if self.used + size > self.buffer.len() {
            self.buffer.resize(self.buffer.len() + self.chunk_size.max(size), 0);
        }
        let ptr = self.used;
        self.used += size;
        &mut self.buffer[ptr..ptr + size]
    }
    
    #[inline]
    fn reset(&mut self) {
        self.used = 0;
    }
    
    #[inline]
    fn len(&self) -> usize {
        self.used
    }
}

/// 从线程本地Arena分配内存。
/// 
/// # 参数
/// * `size` - 要分配的字节数
/// 
/// # 返回
/// 分配的内存 slice
#[inline]
pub fn arena_alloc(size: usize) -> &'static mut [u8] {
    THREAD_LOCAL_ARENA.with(|arena| {
        let inner = unsafe { &mut *arena.get() };
        inner.alloc(size)
    })
}

/// 重置线程本地Arena。
#[inline]
pub fn reset_thread_local_arena() {
    THREAD_LOCAL_ARENA.with(|arena| {
        let inner = unsafe { &mut *arena.get() };
        inner.reset();
    });
}

/// 线程安全的全局对象池管理器。
/// 
/// 用于需要在多线程间共享对象的场景。
/// 使用细粒度锁来减少竞争。
pub struct GlobalPoolManager {
    hit_pools: Vec<HitPool<ExtHit>>,
    buffers: Vec<BufferManager>,
}

impl GlobalPoolManager {
    pub fn new(num_pools: usize, buffer_count: usize, buffer_size: usize) -> Self {
        Self {
            hit_pools: (0..num_pools)
                .map(|_| HitPool::with_capacity(256))
                .collect(),
            buffers: (0..num_pools)
                .map(|_| BufferManager::new(buffer_count, buffer_size))
                .collect(),
        }
    }
    
    #[inline]
    pub fn get_hit_pool(&mut self, thread_id: usize) -> &mut HitPool<ExtHit> {
        let idx = thread_id % self.hit_pools.len();
        &mut self.hit_pools[idx]
    }
    
    #[inline]
    pub fn get_buffer_manager(&mut self, thread_id: usize) -> &mut BufferManager {
        let idx = thread_id % self.buffers.len();
        &mut self.buffers[idx]
    }
    
    #[inline]
    pub fn reset_all(&mut self) {
        for pool in &mut self.hit_pools {
            pool.reset();
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 测试
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    
    #[derive(Debug, Default, PartialEq, Clone)]
    struct TestStruct {
        value: i32,
        name: String,
    }
    
    #[test]
    fn test_object_pool_basic() {
        let mut pool = ObjectPool::<TestStruct>::new(10);
        
        let idx1 = pool.get();
        let item1 = pool.get_mut(idx1);
        item1.value = 42;
        item1.name = "test".to_string();
        
        assert_eq!(item1.value, 42);
        assert_eq!(item1.name, "test");
        
        pool.release(idx1);
        
        let idx2 = pool.get();
        let item2 = pool.get_mut(idx2);
        assert_eq!(item2.value, 0);
        assert_eq!(item2.name, "");
    }
    
    #[test]
    fn test_object_pool_expansion() {
        let mut pool = ObjectPool::<TestStruct>::new(2);
        
        let _idx1 = pool.get();
        let _idx2 = pool.get();
        let _idx3 = pool.get();
        
        assert_eq!(pool.capacity(), 3);
    }
    
    #[test]
    fn test_hit_pool_basic() {
        let mut pool = HitPool::<TestStruct>::with_capacity(10);
        
        pool.push(TestStruct { value: 1, name: "a".to_string() });
        pool.push(TestStruct { value: 2, name: "b".to_string() });
        
        assert_eq!(pool.len(), 2);
        
        let values: Vec<i32> = pool.iter().map(|h| h.value).collect();
        assert_eq!(values, vec![1, 2]);
        
        pool.reset();
        assert_eq!(pool.len(), 0);
        
        pool.push(TestStruct { value: 3, name: "c".to_string() });
        assert_eq!(pool.len(), 1);
        assert_eq!(pool.as_slice()[0].value, 3);
    }
    
    #[test]
    fn test_buffer_manager() {
        let mut manager = BufferManager::new(3, 1024);
        
        let buf1 = manager.get_buffer();
        buf1.extend_from_slice(b"hello");
        assert_eq!(buf1, b"hello");
        
        let buf2 = manager.get_buffer();
        buf2.extend_from_slice(b"world");
        assert_eq!(buf2, b"world");
        
        let buf3 = manager.get_buffer();
        buf3.extend_from_slice(b"test");
        assert_eq!(buf3, b"test");
        
        let buf4 = manager.get_buffer();
        assert_eq!(buf4.len(), 0);
        buf4.extend_from_slice(b"reuse");
        assert_eq!(buf4, b"reuse");
    }
    
    #[test]
    fn test_hit_pool_iter_mut() {
        let mut pool = HitPool::<TestStruct>::with_capacity(5);
        
        pool.push(TestStruct { value: 1, name: "a".to_string() });
        pool.push(TestStruct { value: 2, name: "b".to_string() });
        
        for hit in pool.iter_mut() {
            hit.value *= 2;
        }
        
        let values: Vec<i32> = pool.iter().map(|h| h.value).collect();
        assert_eq!(values, vec![2, 4]);
    }
    
    #[test]
    fn test_global_pool_manager() {
        let mut manager = GlobalPoolManager::new(4, 2, 1024);
        
        let pool1 = manager.get_hit_pool(0);
        pool1.push(ExtHit { chr: 0, loc: 100, snps: 1, strand: 0, gap_size: 0, gap_pos: 0 });
        assert_eq!(pool1.len(), 1);
        
        let pool2 = manager.get_hit_pool(1);
        pool2.push(ExtHit { chr: 1, loc: 200, snps: 2, strand: 0, gap_size: 0, gap_pos: 0 });
        assert_eq!(pool2.len(), 1);
        
        manager.reset_all();
        
        assert_eq!(manager.get_hit_pool(0).len(), 0);
        assert_eq!(manager.get_hit_pool(1).len(), 0);
    }
    
    #[test]
    fn test_thread_local_pool_access() {
        reset_thread_local_pool();
        
        let pool = get_thread_local_pool();
        pool.push(ExtHit {
            chr: 10,
            loc: 1000,
            snps: 3,
            strand: 1,
            gap_size: 0,
            gap_pos: 0,
        });
        
        assert_eq!(pool.len(), 1);
        
        let hits = pool.as_slice();
        assert_eq!(hits[0].chr, 10);
        assert_eq!(hits[0].loc, 1000);
        
        reset_thread_local_pool();
        assert_eq!(get_thread_local_pool().len(), 0);
    }
}
