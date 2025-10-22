use std::collections::HashMap;
use std::time::Instant;

fn main() {
    // Prepare data
    let arr: [u32; 10] = [0,1,2,3,4,5,6,7,8,9];
    let map: HashMap<u32, u32> = (0..10).map(|i| (i, i)).collect();

    // Number of lookups per loop
    const N: usize = 5_000_000;

    // Warmup
    for _ in 0..1_000 { let _ = arr[5]; let _ = map.get(&5); }

    // Benchmark array
    let t0 = Instant::now();
    for _ in 0..N {
        // simple bound-checked indexing (optimized to a single load+check)
        let _ = unsafe { *arr.get_unchecked(5) };
    }
    let arr_ns = t0.elapsed().as_nanos() as f64 / N as f64;

    // Benchmark HashMap
    let t1 = Instant::now();
    for _ in 0..N {
        // &u32 -> hash -> bucket lookup -> pointer chase -> compare
        let _ = map.get(&5);
    }
    let map_ns = t1.elapsed().as_nanos() as f64 / N as f64;

    println!("avg per lookup (ns):\n  array[5]  = {:>6.2}\n  HashMap   = {:>6.2}", arr_ns, map_ns);
}