use ringbuf::traits::{Consumer, Observer, Producer, Split};
use ringbuf::{HeapCons, HeapProd, HeapRb};
use std::sync::{Arc, Mutex};

pub struct SampleRing {
    rb: Arc<Mutex<(HeapProd<f32>, HeapCons<f32>)>>,
}

impl SampleRing {
    pub fn new(capacity: usize) -> Self {
        let rb = HeapRb::<f32>::new(capacity);
        let (p, c) = rb.split();
        Self {
            rb: Arc::new(Mutex::new((p, c))),
        }
    }
}

impl Clone for SampleRing {
    fn clone(&self) -> Self {
        Self {
            rb: Arc::clone(&self.rb),
        }
    }
}

impl SampleRing {
    pub fn push_interleaved(&self, samples: &[f32], channels: usize) -> usize {
        if channels == 0 {
            return 0;
        }
        let mut g = self.rb.lock().unwrap();
        let (p, _) = &mut *g;
        let mut n = 0usize;
        if channels == 1 {
            for &s in samples {
                if p.try_push(s).is_err() {
                    break;
                }
                n += 1;
            }
            return n;
        }
        for chunk in samples.chunks(channels) {
            let mut acc = 0.0f32;
            for &s in chunk {
                acc += s;
            }
            let m = acc / (channels as f32);
            if p.try_push(m).is_err() {
                break;
            }
            n += 1;
        }
        n
    }
    pub fn pop_into(&self, dst: &mut [f32]) -> usize {
        let mut g = self.rb.lock().unwrap();
        let (_, c) = &mut *g;
        c.pop_slice(dst)
    }
    pub fn available(&self) -> usize {
        let g = self.rb.lock().unwrap();
        let (_, c) = &*g;
        c.occupied_len()
    }
}
