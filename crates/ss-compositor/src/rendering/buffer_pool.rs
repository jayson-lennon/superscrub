//! Typestate-governed buffer pool for warp output reuse.
//!
//! Provides zero-allocation buffer reuse for warp output across frames. Each buffer is
//! tracked through a typestate lifecycle: [`Ready`] (usable) → [`Empty`] (receipt for
//! return) → [`Ready`] (returned to pool). The typestate pattern enforces at compile time
//! that buffers are not used after extraction and must be returned.

use std::marker::PhantomData;

/// Marker: the pooled buffer contains data and is ready for use.
pub struct Ready;

/// Marker: the pooled buffer's data has been extracted; this is a receipt for return.
pub struct Empty;

/// A buffer from the pool, tracked through a typestate lifecycle.
///
/// - [`PooledBuffer<Ready>`]: contains a `Vec<u8>`, usable for warp output.
/// - [`PooledBuffer<Empty>`]: a receipt indicating the buffer was extracted; used to
///   return the buffer to the pool.
pub struct PooledBuffer<State> {
    buf: Option<Vec<u8>>,
    expected_size: usize,
    _state: PhantomData<State>,
}

impl<State> std::fmt::Debug for PooledBuffer<State> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PooledBuffer")
            .field("expected_size", &self.expected_size)
            .finish_non_exhaustive()
    }
}

impl PooledBuffer<Ready> {
    /// Extract the inner `Vec<u8>` and produce a receipt for returning it to the pool.
    ///
    /// The caller takes ownership of the vec data. The returned `PooledBuffer<Empty>`
    /// receipt must be used to return the buffer via [`PooledBuffer::replace`].
    pub fn take(mut self) -> (Vec<u8>, PooledBuffer<Empty>) {
        let vec = self
            .buf
            .take()
            .expect("PooledBuffer<Ready> always contains a Vec");
        let size = vec.len();
        let receipt = PooledBuffer {
            buf: None,
            expected_size: size,
            _state: PhantomData::<Empty>,
        };
        (vec, receipt)
    }

    /// Returns the capacity of the contained buffer without extracting it.
    pub fn capacity(&self) -> usize {
        self.expected_size
    }
}

impl PooledBuffer<Empty> {
    /// Return a vec into this receipt, producing a `PooledBuffer<Ready>`.
    ///
    /// # Errors
    ///
    /// Returns [`PooledBufferError`] if the provided vec's length does not
    /// match the expected size recorded when the buffer was extracted. The rejected vec
    /// is returned inside the error variant so the caller can drop or repurpose it.
    pub fn replace(self, buf: Vec<u8>) -> Result<PooledBuffer<Ready>, PooledBufferError> {
        if buf.len() != self.expected_size {
            return Err(PooledBufferError::new(buf));
        }
        Ok(PooledBuffer {
            buf: Some(buf),
            expected_size: self.expected_size,
            _state: PhantomData::<Ready>,
        })
    }

    /// Returns the size that a returned vec must have.
    pub fn expected_size(&self) -> usize {
        self.expected_size
    }
}

/// Error produced when a buffer is returned to the pool with the wrong size.
#[derive(Debug, wherror::Error)]
#[error("wrong buffer size returned to pool")]
pub struct PooledBufferError {
    /// The rejected vec. The caller may drop it or repurpose it.
    pub rejected: Vec<u8>,
}

impl PooledBufferError {
    /// Create a new error with the rejected buffer.
    fn new(rejected: Vec<u8>) -> Self {
        Self { rejected }
    }
}

/// A thread-safe pool of reusable byte buffers.
///
/// Buffers are acquired for warp output and returned after compositing. The pool
/// stores up to `max_size` buffers; excess returns are silently dropped.
///
/// Thread safety: internal storage is protected by `parking_lot::Mutex`. Safe for
/// concurrent use from rayon threads.
pub struct BufferPool {
    /// Internal buffer storage.
    buffers: parking_lot::Mutex<Vec<Vec<u8>>>,
    /// Maximum number of buffers to retain.
    max_size: usize,
}

impl BufferPool {
    /// Create a new pool that retains up to `max_size` buffers.
    pub fn new(max_size: usize) -> Self {
        Self {
            buffers: parking_lot::Mutex::new(Vec::new()),
            max_size,
        }
    }

    /// Acquire a buffer of exactly `size` bytes.
    ///
    /// Reuses a pooled buffer if one is available (exact size match), otherwise
    /// allocates a new one.
    pub fn acquire(&self, size: usize) -> PooledBuffer<Ready> {
        let mut buffers = self.buffers.lock();

        // Search for an exact-size buffer.
        if let Some(idx) = buffers.iter().position(|b| b.len() == size) {
            let buf = buffers.swap_remove(idx);
            return PooledBuffer {
                buf: Some(buf),
                expected_size: size,
                _state: PhantomData::<Ready>,
            };
        }

        // No matching buffer — allocate new.
        PooledBuffer {
            buf: Some(vec![0u8; size]),
            expected_size: size,
            _state: PhantomData::<Ready>,
        }
    }

    /// Release a buffer back to the pool.
    ///
    /// If the pool is at capacity, the oldest buffer is dropped to make room.
    pub fn release(&self, buf: PooledBuffer<Ready>) {
        let vec = buf.buf.expect("PooledBuffer<Ready> always contains a Vec");
        let mut buffers = self.buffers.lock();

        if buffers.len() >= self.max_size {
            // Drop the oldest to make room.
            buffers.remove(0);
        }
        buffers.push(vec);
    }

    /// Returns the number of buffers currently in the pool.
    #[cfg(test)]
    fn len(&self) -> usize {
        self.buffers.lock().len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn acquire_creates_new_buffer_when_pool_empty() {
        // Given a new BufferPool with max_size=4.
        let pool = BufferPool::new(4);

        // When acquiring a buffer of size 100.
        let buf = pool.acquire(100);

        // Then the buffer has capacity 100 and contains all zeros.
        assert_eq!(buf.capacity(), 100);
        let (vec, _) = buf.take();
        assert_eq!(vec.len(), 100);
        assert!(
            vec.iter().all(|&b| b == 0),
            "buffer should be zero-initialized"
        );
    }

    #[test]
    fn acquire_reuses_returned_buffer() {
        // Given a pool with a returned buffer of size 100.
        let pool = BufferPool::new(4);
        let buf = pool.acquire(100);
        let (mut vec, receipt) = buf.take();
        // Fill with non-zero data to verify reuse.
        vec.fill(0xAB);
        let ready = receipt.replace(vec).expect("same size should succeed");
        pool.release(ready);
        assert_eq!(pool.len(), 1);

        // When acquiring a buffer of size 100.
        let buf = pool.acquire(100);

        // Then the pool reuses the returned buffer.
        assert_eq!(
            pool.len(),
            0,
            "pool should be empty after reusing the buffer"
        );
        let (vec, _) = buf.take();
        assert!(
            vec.iter().all(|&b| b == 0xAB),
            "reused buffer should contain previous data"
        );
    }

    #[test]
    fn replace_rejects_wrong_size() {
        // Given an Empty receipt expecting size 100.
        let pool = BufferPool::new(4);
        let buf = pool.acquire(100);
        let (_, receipt) = buf.take();
        assert_eq!(receipt.expected_size(), 100);

        // When replacing with a vec of size 50.
        let result = receipt.replace(vec![0u8; 50]);

        // Then the result is Err(PooledBufferError) containing the rejected vec.
        let err = result.expect_err("wrong size should fail");
        assert_eq!(err.rejected.len(), 50, "rejected vec should be size 50");
    }

    #[test]
    fn pool_drops_oldest_at_capacity() {
        // Given a pool with max_size=2 containing buffers of sizes [100, 200].
        let pool = BufferPool::new(2);
        pool.release(pool.acquire(100));
        pool.release(pool.acquire(200));

        // When releasing a third buffer of size 300.
        pool.release(pool.acquire(300));

        // Then the pool contains [200, 300] (oldest dropped).
        let buffers = pool.buffers.lock();
        assert_eq!(buffers.len(), 2);
        assert_eq!(
            buffers[0].len(),
            200,
            "oldest (100) should be dropped, 200 remains"
        );
        assert_eq!(buffers[1].len(), 300);
    }

    #[test]
    fn replace_accepts_correct_size() {
        // Given an Empty receipt expecting size 100.
        let pool = BufferPool::new(4);
        let buf = pool.acquire(100);
        let (_, receipt) = buf.take();

        // When replacing with a vec of size 100 filled with 0xAB.
        let result = receipt.replace(vec![0xAB; 100]);

        // Then the result is Ok(PooledBuffer<Ready>) and the buffer contains 0xAB bytes.
        let ready = result.expect("same size should succeed");
        let (vec, _) = ready.take();
        assert!(
            vec.iter().all(|&b| b == 0xAB),
            "buffer should contain 0xAB bytes"
        );
    }
}
