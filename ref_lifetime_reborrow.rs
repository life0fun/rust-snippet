use core::ptr::NonNull;
use std::sync::atomic::AtomicPtr;
// https://users.rust-lang.org/t/why-is-the-mutable-borrow-not-dropped-at-end-of-the-block/32441/7

// &mut: unqiue/exclusive reference. &, aliasable reference.
// mutation only safe with exclusive borrow. AtomicPtr, CAS allows change using sharable ref.
//https://stackoverflow.com/questions/35810843/why-do-rusts-atomic-types-use-non-mutable-functions-to-mutate-the-value

// It is the lifetime of the reference that decides how long it is borrowed.

// https://stackoverflow.com/questions/38713228/cannot-borrow-variable-when-borrower-scope-ends
mod Foo {
  trait Foo<'a> {
    fn new(data: &'a mut u32) -> Self;
    fn apply(&mut self);
  }
  struct FooS<'a> {
    data: &'a mut u32
  }

  impl<'a> Foo<'a> for FooS<'a> {
    fn new(data: &'a mut u32) -> Self {
      FooS { data: data }
    }
    fn apply(&mut self) {
        *self.data += 10;
    }
  }
  fn test<F: Foo>(data: &'a mut u32)
  where F: Foo<'a>
  {
      { // regional &'b < 'a can be chosen as it is not a function call.
        //let mut foo = FooS {data: data};
        
        // Fn call, 'a has to be used to pass the new()
        let mut foo = Foo::new(data);
        foo.apply();
      }
      println!("{:?}", data); // error
  }
  // dynamic dispatch with trait object
  fn test_foo(f: &mut dyn Foo) {
      f.apply();
  }

  fn main() {
    let mut a = 10;
    test::<FooS>(&mut a);
    println!("out {:?}", a)
  }
}

mod borrow_reborrow {
    use core::ptr::NonNull;
    use std::sync::atomic::AtomicPtr;
    use std::sync::atomic::Ordering;

    pub fn move_or_reborrow_mut_ref() {
        let mut x = 10;
        {
            let x_share_ref = &x;
            let x_share_ref2 = x_share_ref; // share a shared-borrow to another ref
            assert_eq!(*x_share_ref, 10);

            let x_mut_ref = &mut x;
            let x_mut_ref_2 = x_mut_ref; // moved exclusive mut borrow
            // assert_eq!(*x_mut_ref, 10);  // x_mut_ref moved, can not use here.
            assert_eq!(x, 10);
        }
        {
            let x_mut_ref = &mut x;
            // a mut ref can be re-borrowed
            let x_mut_ref_2 = &mut *x_mut_ref; // explicit re-borrow
            let x_mut_ref_3: &mut i32 = x_mut_ref; // exclusively mut re-borrow by coerce from x_ref

            // can not touch x_mut_ref after it being exclusively mut re-borrowed.
            // println!("x_mut_ref = {} ", x_mut_ref);

            *x_mut_ref_3 = 12;
            assert_eq!(*x_mut_ref_3, 12);
            // assert_eq!(*x_mut_ref_2, 12);
            assert_eq!(*x_mut_ref, 12);
        }
        println!("x = {}", x);
    }
    pub fn reborrow() {
        let mut data = "hello".to_string();
        let ref1 = &mut data; // data is write locked after borrow to ref1.
        let ref2 = &mut *ref1; // ref1 is write locked after reborrow to ref2

        // can not mut data until its mut borrow/lock to ref1 is dropped.
        // data = "hello again".to_string();

        // &mut is a var(pointer) holds the ownership. Like a AtomicPtr.
        // any var that gives out mut borrow/WLocked, can not reborrow or move(=)
        // can not mov out of ref1 unless its reborrowed to ref2 is dropped.
        // let ref3 = ref1; // &mut var is also guarded by move/borrow lock.

        // ref2 is ownning the data, only it can mutate data.
        *ref2 = "hello by ref2".to_string();
        println!("{}", *ref2);

        // can not mutate ref1 until its mut borrow/lock to ref2 is dropped.
        // *ref1 = "hello by ref1".to_string();
        // can not borrow data until its mut borrow is dropped.
        // println!("{}", data);

        *ref2 = "hello by ref2".to_string();
        println!("{}", *ref2);

        // ref2 dropped after use. now ref1 owns data.
        *ref1 = "hello by ref1".to_string();
        // can not borrow data until its mut borrow to ref1 is dropped.
        println!("{}", data);

        // now ref1 is dropped, data is solely owned.
        data = "hello again".to_string();
        println!("{}", data);
    }
    pub fn ref_as_pointer() {
        let mut x = 10;
        let x_ptr = &mut x as *mut i32;
        assert!(!x_ptr.is_null());

        unsafe { *x_ptr = 11 };
        assert_eq!(x, 11);

        // Plain-Old-data, just no move ownership.
        let x_ptr2 = x_ptr;
        unsafe { *x_ptr = 12 }; // *x_ptr = 11;
        unsafe { *x_ptr2 = 13 }; // *x_ptr = 11;
        assert_eq!(unsafe { *x_ptr }, 13);
        assert_eq!(x, 13);

        let mut x_ptr = &mut x as *mut i32;
        // no move, but the mut borrow on the var still exclusive.
        let x_ptr3 = &mut x_ptr;
        unsafe { *x_ptr = 12 }; // *x_ptr = 11;
        assert_eq!(unsafe { *x_ptr }, 12);
        // assert_eq!(unsafe {**x_ptr3}, 12);
        let x_ptr4 = &mut x_ptr;
        assert_eq!(unsafe { **x_ptr4 }, 12);

        let mut x_const_ptr = &x as *const i32;
        let mut x_mut_ptr = &mut x as *mut i32;

        x_const_ptr = x_mut_ptr as *const i32;
        x_mut_ptr = x_const_ptr as *mut i32;
    }

    pub fn cast_ptr() {
        let mut x = 10;
        let mut x_const_ptr = &x as *const i32;
        let mut x_mut_ptr = &mut x as *mut i32;

        x_const_ptr = x_mut_ptr as *const i32;
        x_mut_ptr = x_const_ptr as *mut i32;
    }

    pub fn nonnull() {
        let mut x = 10;
        let mut ptr = NonNull::<i32>::new(&mut x as *mut _).expect("non null is valid");
        // no move on <=
        let mut ptr2 = ptr; // no move
        assert_eq!(unsafe { *ptr.as_ptr() }, 10);
        assert_eq!(unsafe { *ptr2.as_ptr() }, 10);
        // mut
        unsafe { *ptr2.as_mut() += 1 };
        assert_eq!(unsafe { *ptr.as_ptr() }, 11);

        // mut borrow on the var still exclusive
        let ptr3 = &mut ptr;
        unsafe { *ptr.as_mut() += 1 };
        assert_eq!(unsafe { *ptr.as_ptr() }, 12);
        // assert_eq!(unsafe {*ptr3.as_ptr()}, 12);

        let ptr4 = &mut ptr;
        unsafe { *ptr4.as_mut() += 1 };
        assert_eq!(unsafe { *ptr4.as_ptr() }, 13);

        let mut s = "hell".to_string();
        let mut ptr = unsafe { NonNull::new_unchecked(&mut s) };
        let ref1 = &mut ptr; // even copyable value is guarded by borrow rules.
                             // can not use it after mutably borrowed, though the ptr is copyable.
                             //unsafe {*ptr.as_ptr() = "world".to_string(); }
        println!("ref1={:?}", ref1);

        // NonNull<T> is copyable. Option<NonNull<T> also copyable.
        let mut cp1 = ptr;
        let mut cp2 = ptr;

        let mut cp1ref = unsafe { cp1.as_mut() };
        let mut cp2ref = unsafe { cp2.as_mut() };
        let cp1refre = &mut cp1ref;
        //println!("cp1ref={:?}", cp1ref);
        println!("cp1refre={:?}", cp1refre);

        s = "world".to_string();
        unsafe {
            println!(
                "ptr={:?} cp1={:?} cp2={:?}",
                ptr.as_mut(),
                cp1.as_mut(),
                cp2.as_mut()
            );

            **cp1refre = "lleh".to_string();
            println!("ptr={:?}", ptr.as_mut());
        }
    }

    pub fn atomic_ptr() {
        let mut x = 10;
        let a_ptr = AtomicPtr::new(&mut x);
        let a_ptr2 = a_ptr; // move

        let new_ptr = &mut 11;
        let mut old_ptr = a_ptr2.load(Ordering::SeqCst);
        loop {
            match a_ptr2.compare_exchange_weak(old_ptr, new_ptr, Ordering::SeqCst, Ordering::SeqCst)
            {
                Ok(_) => {
                    break;
                }
                Err(cur_now) => {
                    old_ptr = cur_now;
                }
            }
        }
        assert_eq!(unsafe { *a_ptr2.load(Ordering::SeqCst) }, 11);

        {
            // unsafe store into AtomicPtr via shared ref.
            let mut data = 10;
            let mut atomic_ptr = AtomicPtr::new(&mut data);
            let mut new_data = 5;

            let ptr2 = &atomic_ptr;
            // safe with mutable reference
            // *atomic_ptr.get_mut() = &mut other_data;

            // unsafe via shared pointer
            unsafe { ptr2.store(&mut new_data, Ordering::SeqCst) };
            assert_eq!(unsafe { *atomic_ptr.load(Ordering::SeqCst) }, 5);
            assert_eq!(unsafe { *ptr2.load(Ordering::SeqCst) }, 5);
        }
    }
}
mod variance {
// decouple the mut borrow of str from the lifetime of &str.
fn strtok<'a, 'b>(s: &'b mut &'a str, delimit: char) -> &'a str {
    if let Some(i) = s.find(delimit) {
        let prefix = &s[..i];
        let suffix = &s[(i + 1)..];
        *s = suffix;
        prefix
    } else {
        let prefix = *s;
        *s = "";
        prefix
    }
}

// the returned str is the same lifetime of the passed in str, the lifetime of borrow can be shortned.
// fn strtok_covariant<'a>(s: &mut &'a str, delimit: char) -> &'a str {
  
// mutable borrow is the same lifetime of the returned str.
fn strtok_covariant<'a>(s: &'a mut &'a str, delimit: char) -> &'a str {
    if let Some(i) = s.find(delimit) {
        let prefix = &s[..i];
        let suffix = &s[(i + 1)..];
        *s = suffix;
        prefix
    } else {
        let prefix = *s;
        *s = "";
        prefix
    }
}

pub fn test() {
  {
      let mut hello_str = "hello world";
      let prefix = strtok_covariant(&mut hello_str, ' ');
      // mutable borrow of hello_str lives as long as the returned prefix is in scope.
      // hence the immut borrow of hello_str is invalid.
      assert_eq!(hello_str, "world");
      assert_eq!(prefix, "hello");
  }
  {
      let mut hello_str = "hello world";
      let prefix = strtok(&mut hello_str, ' ');
      assert_eq!(hello_str, "world");
      assert_eq!(prefix, "hello");
  }
}
}

mod mut_borrow_lifetime {
  struct MyIterWrapper<'a, T> {
    slice: &'a [T],
  }

  impl<'a, T> Iterator for MyIterWrapper<'a, T> {
    type Item =&'a T;
    fn next(&mut self) -> Option<Self::Item> {
        let (element, rest) = self.slice.split_first()?;
        self.slice = rest;
        Some(element)
    }
  }

  struct MyMutIterWrapper<'iter, T> {
    slice: &'iter mut [T],
  }
  impl<'iter, T> Iterator for MyMutIterWrapper<'iter, T> {
    type Item = &'iter mut T;
    fn next<'next>(&'next mut self) -> Option<Self::Item> {
        //let mut slice = unsafe { std::mem::transmute::<&'next mut [T], &'iter mut [T]>(&mut self.slice) };
        let slice = std::mem::replace(&mut self.slice, &mut []);
        let (first, rest) = slice.split_first_mut()?;
        self.slice = rest;
        Some(first)
    }
  }

  #[cfg(test)]
  mod tests {
    use super::*;
    
    #[test]
    fn it_works() {
        let mut collection = vec![1,2,3,4];
        let wrapper = MyIterWrapper {
            slice: &collection[..],
        };
        for (index, elem) in wrapper.enumerate() {
            assert_eq!(*elem, collection[index]);
        }
    }
    #[test]
    fn mut_iter_works() {
        let mut collection = vec![1,2,3,4];
        {
            let wrapper = MyMutIterWrapper {
                slice: &mut collection[..],
            };
            for (index, elem) in wrapper.enumerate() {
                *elem += 1;
            }
        }
        assert_eq!(collection[0], 2);
    }
  }
}

fn main() {
    borrow_reborrow::atomic_ptr();
    variance::test();
}
