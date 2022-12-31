use core::time::Duration;
use std::cell::{Cell, RefCell};
use std::sync::{Arc, Mutex, RwLock};
use std::thread;
use std::thread::sleep;
fn test() {
    {
        // can by value send Arc<Vec<T>>, but Arc<Vec<T>> is not sync, so can not push to v.
        let arc_v = Arc::new(vec![1, 2]);
        let clone1 = arc_v.clone();
        // cannot borrow data in an `Arc` as mutable, why ? ref-counting is for sharing,
        // Rust prohibits mutable sharing.
        let t1 = thread::spawn(move || {
            // arc_v.push(3);
            assert_eq!(clone1.as_ref(), &[1, 2]);
        });
        t1.join().unwrap();
        assert_eq!(arc_v.as_ref(), &[1, 2]);
    }
    {
        // stackoverflow.com/questions/67877287/
        // interior mutability Types are not Sync as they are non-thread-safe.
        let arc_v = Arc::new(RefCell::new(vec![1, 2]));
        let clone1 = arc_v.clone();
        // cannot borrow data in an `Arc` as mutable, why ? ref-counting is for sharing,
        // Rust prohibits mutable sharing.
        let t1 = thread::spawn(move || {
            // assert_eq!(clone1.as_ref().take(), &[1,2]);
        });
        t1.join().unwrap();
        assert_eq!(arc_v.as_ref().take(), &[1, 2]);

        let arc_v = Arc::new(Cell::new(vec![1, 2])); // interior mut types not sync, can not be shared.
                                                     // cannot borrow data in an `Arc` as mutable, why ? ref-counting is for sharing,
                                                     // Rust prohibits mutable sharing.
        let t1 = thread::spawn(move || {
            // assert_eq!(arc_v.as_ref().take(), &[1,2]);
        });
        t1.join().unwrap();
        assert_eq!(arc_v.as_ref().take(), &[1, 2]);
    }
    // Arc<T> = shard_prt<T> multiple owners across many threads.
    // To Safe sharing, each thread hold a clone, and Mutex<T> or RwLock::new<T>;
    // The threading equivalent to RefCell is Mutex
    {
        let arc_v = Arc::new(Mutex::new(vec![1, 2]));
        let clone1 = arc_v.clone(); // Arc::clone(&arc_v); // arc_v.clone();
        let clone2 = arc_v.clone(); // Arc::clone(&arc_v); // arc_v.clone();
        let t1 = thread::spawn(move || {
            clone1.lock().unwrap().push(3);
        });
        t1.join().unwrap();
        let t2 = thread::spawn(move || {
            clone2.lock().unwrap().push(4);
        });
        t2.join().unwrap();
        assert_eq!(*arc_v.lock().unwrap(), &[1, 2, 3, 4]);
    }

    {
        let rw_lock = Arc::new(RwLock::new(vec![1, 2]));

        // Each thread owns a clone. Rc clone is shallow. Share the locked data.
        let producer_lock = rw_lock.clone();
        let consumer_id_lock = rw_lock.clone();
        let consumer_square_lock = rw_lock.clone();

        let producer_thread = thread::spawn(move || {
            loop {
                // write() blocks this thread until write-exclusive acquired
                if let Ok(mut write_guard) = producer_lock.write() {
                    // the returned write_guard implements `Deref` giving us easy access to the target value
                    write_guard.push(3);
                    println!("Producer_thread : update value: {:?}", write_guard);
                    return;
                }
                sleep(Duration::from_millis(1000));
            }
        });

        // A reader thread that prints the current value to the screen
        let consumer_id_thread = thread::spawn(move || {
            loop {
                // read() will only block when `producer_thread` is holding a write lock
                if let Ok(read_guard) = consumer_id_lock.read() {
                    // the returned read_guard also implements `Deref`
                    println!("Consumer_thread : read value: {:?}", read_guard);
                    return;
                }
                sleep(Duration::from_millis(500));
            }
        });

        // A second reader thread is printing the squared value
        let consumer_square_thread = thread::spawn(move || loop {
            if let Ok(read_guard) = consumer_square_lock.read() {
                println!(
                    "Consumer_square_thread : read value squared: {:?}",
                    read_guard
                );
                return;
            }
            sleep(Duration::from_millis(750));
        });

        let _ = producer_thread.join();
        let _ = consumer_id_thread.join();
        let _ = consumer_square_thread.join();
    }
}
