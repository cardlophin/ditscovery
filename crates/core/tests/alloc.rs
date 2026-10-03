#![allow(unsafe_code)]

use ditscover_core::config::Config;
use ditscover_core::motif::{Ctx, Scratch, Segment, evaluate};
use ditscover_core::synth;
use serde_json::json;
use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};

struct Counting;

static ALLOCS: AtomicUsize = AtomicUsize::new(0);

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        ALLOCS.fetch_add(1, Ordering::Relaxed);
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }
}

#[global_allocator]
static GLOBAL: Counting = Counting;

#[test]
fn evaluation_hot_loop_does_not_allocate() {
    let (x, truth) = synth::planted(5000, &[80], 10, 0.3, 0.1, 2);
    let segs: Vec<Segment> = truth[0].iter().map(|&(start, len)| Segment { start, len }).collect();
    for kind in ["zeuclid", "euclid", "dtw"] {
        let o = [("min_len", json!(40)), ("max_len", json!(120)), ("kind", json!(kind))];
        let cfg = Config::build(None, None, &o.map(|(k, v)| (k.to_string(), v))).unwrap();
        let ctx = Ctx::new(&x, &cfg);
        let mut s = Scratch::new(120);
        evaluate(&segs, &ctx, &mut s);
        let before = ALLOCS.load(Ordering::Relaxed);
        for _ in 0..100 {
            std::hint::black_box(evaluate(&segs, &ctx, &mut s));
        }
        assert_eq!(ALLOCS.load(Ordering::Relaxed), before, "{kind} allocated");
    }
}
