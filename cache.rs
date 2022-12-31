// https://stackoverflow.com/questions/71012845
// https://github.com/xacrimon/dashmap
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};

// self-referential struct.
// The Pin type wraps pointer types, guaranteeing that the
// values behind the pointer won't be moved.
// For example, Pin<&mut T>, Pin<&T>, Pin<Box<T>> all guarantee
// that T won't be moved even if T: !Unpin.
#[derive(Debug, Clone)]
struct RefTestStruct {
    key: usize,
    _ref_value: Arc<String>,
}

type Cache = HashMap<usize, (Arc<String>, RefTestStruct)>;
type AmCache = Arc<Mutex<Cache>>;

fn init(cache: &AmCache) {
    let mut handles: Vec<JoinHandle<()>> = vec![];
    for idx in 0..10_usize {
        //create reference copy of cache
        let cache_clone = Arc::clone(cache);

        let handle = thread::spawn(move || {
            //lock cache
            let mut locked_cache = cache_clone.lock().unwrap();

            // add new object to cache
            let s = Arc::new(format!("value: {}", idx));
            let ref_struct = RefTestStruct {
                key: idx,
                _ref_value: s.clone(),
            };
            let tuple_value = (s, ref_struct);
            locked_cache.insert(idx, tuple_value);

            println!(
                "IDX: {} - CACHE: {:?}",
                idx,
                locked_cache.get(&idx).unwrap()
            )
        });

        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    println!("\n");
}

pub fn main() {
    // init
    let cache = Cache::new();
    let am_cache = Arc::new(Mutex::new(cache));

    init(&am_cache);

    // change cache contents
    let mut handles: Vec<JoinHandle<()>> = vec![];
    for idx in 0..10_usize {
        let cache_clone = Arc::clone(&am_cache);

        let handle = thread::spawn(move || {
            let mut locked_cache = cache_clone.lock().unwrap();
            let tuple_value = locked_cache.get_mut(&idx).unwrap();
            let new_key = tuple_value.1.key + 10;
            let new_s = Arc::new(format!("changed value: {}", new_key));
            (*tuple_value).1 = RefTestStruct {
                key: new_key,
                _ref_value: new_s.clone(),
            };
            (*tuple_value).0 = new_s;
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    // display cache contents
    let mut handles: Vec<JoinHandle<()>> = vec![];
    for idx in 0..10_usize {
        let cache_clone = Arc::clone(&am_cache);

        let handle = thread::spawn(move || {
            let locked_cache = cache_clone.lock().unwrap();
            println!("===== thread {} ===== ", idx);
            for idx in 0..10_usize {
                let ts_obj = locked_cache.get(&idx).unwrap();
                // println!("IDX: {} - CACHE: {:?}", idx, &ts_obj);
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }
}
