// https://tekshinobi.com/rust-tips-box-rc-arc-cell-refcell-mutex/

// Rc let you hold a shared ref without worry lifetime as ref counting.
// Cell<T> let you mutate(setter) via a shared ref.  { state: Cell<RefState> }
// RefCell<T> use UnsafeCell to store value and Cell<RefState> to track ref counts using unsafe { &*self }

mod MyRc {
    use core::marker::PhantomData;
    use core::ptr::NonNull;
    use std::cell::Cell;

    struct RcInner<T> {
        value: T,
        refcount: Cell<usize>,
    }
    pub struct MyRc<T> {
        inner: NonNull<RcInner<T>>,
        _marker: PhantomData<RcInner<T>>,
    }
    impl<T> MyRc<T> {
        pub fn new(value: T) -> Self {
            let inner = Box::new(RcInner {
                value,
                refcount: Cell::new(1),
            });
            MyRc {
                inner: unsafe { NonNull::new_unchecked(Box::into_raw(inner)) },
                _marker: PhantomData,
            }
        }
    }
    impl<T> std::ops::Deref for MyRc<T> {
        type Target = T;
        fn deref(&self) -> &Self::Target {
            let inner = unsafe { self.inner.as_ref() };
            &inner.value
        }
    }
    impl<T> Clone for MyRc<T> {
        fn clone(&self) -> Self {
            let inner = unsafe { self.inner.as_ref() };
            inner.refcount.set(inner.refcount.get() + 1);
            MyRc {
                inner: self.inner,
                _marker: PhantomData,
            }
        }
    }
    impl<T> Drop for MyRc<T> {
        fn drop(&mut self) {
            let inner = unsafe { self.inner.as_ref() };
            let c = inner.refcount.get();
            if c == 1 {
                drop(inner);
                let _ = unsafe { Box::from_raw(self.inner.as_ptr()) };
            } else {
                inner.refcount.set(c - 1);
            }
        }
    }
}

use core::time::Duration;
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::{Arc, Mutex, RwLock};
use std::thread;
use std::thread::sleep;
fn test() {
    {
        // Rc itself can’t be dereferenced mutably; get_mut().unwrap() iff ref count = 1;
        // Rc is shard_ptr<const T>; Rc<RefCell<T>> shared with interior mut on inner most data.
        // allows mutate data even you have a immu ref(&T) to outer RefCell<T> var binding.
        // Rc<RefCell<Vec<T>> inside to get dynamically verified shared mutability.
        // useful in nested node tree where adjacent nodes/edges shared owned by nodes.
        {
            type NodeRef<T> = Rc<RefCell<_Node<T>>>;
            // The private representation of a node.
            struct _Node<T> {
                inner_value: T,
                adjacent: Vec<NodeRef<T>>,
            }
            struct Node<T>(NodeRef<T>);
        }
        let v = vec![String::from("a")];
        let rc = Rc::new(RefCell::new(v)); // v moved into RefCell.
        let rc_clone = rc.clone(); // Rc clone is shallow.
                                   // one variable binding can only have a single mutable borrow.
                                   // Rc's clone is shallow, appending happen to Rc's RefCell.
        rc.borrow_mut()[0].push_str(":b");
        rc.borrow_mut().push(String::from("b"));
        rc_clone.borrow_mut().push(String::from("c"));
        // Borrow::borrow() trait vs. RefCell::borrow().
        // assert_eq!(*rc.borrow(), &["a", "b", "c"]);
        assert_eq!(*rc.as_ref().borrow(), &["a:b", "b", "c"]);
        assert_eq!(*rc_clone.as_ref().borrow(), &["a:b", "b", "c"]);
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

fn main() {
    test();
}
