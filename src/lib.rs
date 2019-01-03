#![feature(atomic_min_max)]

use std::sync::atomic::{AtomicUsize, AtomicBool, Ordering};
use std::ops::{Deref, DerefMut};
use std::any::Any;
use std::sync::{Arc, Mutex};
use std::collections::VecDeque;

#[derive(Debug)]
pub struct Buffer<T> {
    sequestered: Box<Any>,
    ptr: *mut T,
    len: usize,
}

impl<T> Buffer<T> {
    /// Create a new instance from a byte allocation.
    pub fn from<B>(bytes: B) -> Buffer<T> where B: DerefMut<Target=[T]>+'static {
        let mut boxed = Box::new(bytes) as Box<Any>;

        let ptr = boxed.downcast_mut::<B>().unwrap().as_mut_ptr();
        let len = boxed.downcast_ref::<B>().unwrap().len();

        Buffer {
            ptr,
            len,
            sequestered: boxed,
        }
    }
}

pub struct BlipBufInternal {
    buf: Arc<Buffer<u8>>,
    valid: AtomicUsize,
    extracted: AtomicUsize,
    completed: AtomicBool,
}

// send
#[derive(Clone)]
pub struct BlipBuf {
    internal: Arc<BlipBufInternal>,
}

impl BlipBuf {
    pub fn new(buf: Arc<Buffer<u8>>) -> BlipBuf {
        BlipBuf { internal: Arc::new(BlipBufInternal {
            buf,
            valid: AtomicUsize::new(0),
            extracted: AtomicUsize::new(0),
            completed: AtomicBool::new(false),
        }) }
    }

    pub fn extract_valid(&mut self) -> (Option<Blip>, bool) {
        let completed = self.internal.completed.load(Ordering::Acquire);
        let end = self.internal.valid.load(Ordering::Acquire);
        let start = self.internal.extracted.fetch_max(end, Ordering::SeqCst);
        if end - start > 0 {
            (Some(Blip {
                buf: self.internal.buf.clone(),
                ptr: unsafe { self.internal.buf.ptr.add(start) },
                len: end - start,
            }), completed)
        } else {
            (None, completed)
        }
    }

    pub fn try_regenerate(&mut self) -> Option<BlipBufWriter> {
        if Arc::strong_count(&self.internal) == 1 && Arc::strong_count(&self.internal.buf) == 1 {
            self.internal.valid.store(0, Ordering::Release);
            self.internal.extracted.store(0, Ordering::Release);
            Some(BlipBufWriter { internal: self.internal.clone(), reserved: 0 })
        }
        else {
            None
        }
    }
}

// not sync
pub struct BlipBufWriter {
    internal: Arc<BlipBufInternal>,
    reserved: usize,
}

impl BlipBufWriter {
    pub fn reserve(&mut self, len: usize) -> Option<BlipBufReservation<'_>> {
        // TODO: check reservation
        let start = self.reserved;
        self.reserved += len;
        unsafe {
            Some(BlipBufReservation {
                start,
                len,
                buf: &mut self.internal,
            })
        }
    }
}

pub struct BlipBufReservation<'a> {
    start: usize,
    len: usize,
    buf: &'a mut Arc<BlipBufInternal>,
}

impl<'a> Deref for BlipBufReservation<'a> {
    type Target = [u8];

    #[inline(always)]
    fn deref(&self) -> &[u8] {
        unsafe { ::std::slice::from_raw_parts(self.buf.buf.ptr, self.buf.buf.len) }
    }
}

impl<'a> DerefMut for BlipBufReservation<'a> {
    fn deref_mut(&mut self) -> &mut [u8] {
        unsafe { ::std::slice::from_raw_parts_mut(self.buf.buf.ptr, self.buf.buf.len) }
    }
}

impl<'a> core::ops::Drop for BlipBufReservation<'a> {
    fn drop(&mut self) {
        self.buf.valid.fetch_add(self.len, Ordering::Release);
    }
}

pub struct Blip {
    buf: Arc<Buffer<u8>>,
    ptr: *mut u8,
    len: usize,
}

impl Blip {
    /// Extracts [0, index) into a new `Bytes` which is returned, updating `self`.
    ///
    /// #Safety
    ///
    /// This method uses an `unsafe` region to advance the pointer by `index`. It first
    /// tests `index` against `self.len`, which should ensure that the offset is in-bounds.
    pub fn extract_to(&mut self, index: usize) -> Blip {

        assert!(index <= self.len);

        let result = Blip {
            ptr: self.ptr,
            len: index,
            buf: self.buf.clone(),
        };

        unsafe { self.ptr = self.ptr.offset(index as isize); }
        self.len -= index;

        result
    }
}

impl Deref for Blip {
    type Target = [u8];
    fn deref(&self) -> &[u8] {
        unsafe { ::std::slice::from_raw_parts(self.ptr, self.len) }
    }
}

impl DerefMut for Blip {
    fn deref_mut(&mut self) -> &mut [u8] {
        unsafe { ::std::slice::from_raw_parts_mut(self.ptr, self.len) }
    }
}

pub fn blip_queue(bufs: Vec<BlipBuf>) -> (BlipQueueSender, BlipQueueReceiver) {
    let queue = Arc::new(Mutex::new(VecDeque::new()));
    (
        BlipQueueSender {
            bufs,
            queue: queue.clone(),
            writer: None,
        },
        BlipQueueReceiver {
            queue,
        }
    )
}

pub struct BlipQueueSender {
    bufs: Vec<BlipBuf>,
    queue: Arc<Mutex<VecDeque<BlipBuf>>>,
    writer: Option<BlipBufWriter>,
}

impl BlipQueueSender {
    pub fn reserve_send(&mut self, len: usize) -> BlipBufReservation<'_> {
        // TODO: populate writer
        self.queue.lock().unwrap().push_back(unimplemented!());
        self.writer.as_mut().unwrap().reserve(len).unwrap() // TODO retry on failure
    }
}

pub struct BlipQueueReceiver {
    queue: Arc<Mutex<VecDeque<BlipBuf>>>,
}

impl BlipQueueReceiver {
    pub fn recv(&mut self) -> Blip {
        // TODO clean up completed
        let (out, completed) = self.queue.lock().unwrap().front_mut().as_mut().unwrap().extract_valid();
        out.unwrap()
    }
}
