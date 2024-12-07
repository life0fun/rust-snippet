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
      fn new(data: &'a mut u32) -> Self where Self: Sized;
      fn apply(&mut self);
    }
    struct FooS<'a> {
      data: &'a mut u32  // ref is borrow, stored ref must have lifetime;
    }
    impl<'a> Foo<'a> for FooS<'a> {
      fn new(data: &'a mut u32) -> Self {
        FooS { data: data }
      }
      fn apply(&mut self) {
          *self.data += 10;
      }
    }
    // function lifetime parameter explicitly repr 'a live longer than fn call. 
    fn test<'a, F>(data: &'a mut u32) where F: Foo<'a> {
      let mut foo = FooS::new(data);  // borrow lifetime of narrowed to FooS::new
      // let mut foo: F = Foo::new(data);  // borrow lifetime of 'a 
      foo.apply();
      println!("data is {:?}", data);
    }
    fn test_dyn_foo(f: &mut dyn Foo) {
      f.apply();
    }
  
    pub fn main() {
      let mut a = 10;
      test::<FooS>(&mut a);
      println!("out {:?}", a);
      let mut foo = FooS::new(&mut a);
      test_dyn_foo(&mut foo);
    }
}

mod borrow_reborrow {
    use core::ptr::NonNull;
    use std::sync::atomic::AtomicPtr;
    use std::sync::atomic::Ordering;

    pub fn move_or_reborrow_mut_ref() {
        let mut s = String::from("hello");
        {
            let s_share_ref = &s;
            let s_share_ref2 = s_share_ref; // share a shared-borrow to another ref
            assert_eq!(s_share_ref, "hello");

            let s_mut_ref = &mut s;
            let s_mut_ref2 = s_mut_ref; // moved exclusive mut borrow
            s_mut_ref2.push('1');
            //assert_eq!(s_mut_ref, "hello");  // s_mut_ref binding moved to s_mut_ref2
            assert_eq!(s_mut_ref2, "hello1");  // s_mut_ref binding moved to s_mut_ref2
            assert_eq!(s, "hello1");  // s_mut_ref binding moved to s_mut_ref2
        }
        {
            let mut s = String::from("hello");
            let s_mut_ref = &mut s;
            // a mut ref can be re-borrowed
            let s_mut_ref2 = &mut *s_mut_ref; // explicit re-borrow
            s_mut_ref2.push('2');
            assert_eq!(s_mut_ref2, "hello2");
            assert_eq!(s, "hello2");
        }
        {
            let mut s = String::from("hello");
            let mut s_mut_ref = &mut s;
            // a mut ref can be re-borrowed
            let s_mut_ref2 = &mut s_mut_ref; // explicit re-borrow
            s_mut_ref2.push('2');
            assert_eq!(/*deref=*/*s_mut_ref2, "hello2");
            assert_eq!(s, "hello2");
        }
        {
            let mut s = String::from("hello");
            let mut s_mut_ref = &mut s;
            // a mut ref can be re-borrowed
            let s_mut_ref2 = &mut s_mut_ref; // explicit re-borrow
            s_mut_ref2.push('2');
            assert_eq!(/*deref=*/*s_mut_ref2, "hello2");
            assert_eq!(s, "hello2");
        }
        {
            let mut s = String::from("hello");
            let mut s_mut_ref = &mut s;
            let mut s_mut_ref2 = &mut s_mut_ref;
            // a mut ref can be re-borrowed
            let s_mut_ref3 = &mut s_mut_ref2; // ref3 explicit re-borrow  
            // s_mut_ref2.push('2'); // ref2 already borrowed to ref3, can not re-borrow more than once
            s_mut_ref3.push('2');
            assert_eq!(/*deref=*/**s_mut_ref3, "hello2");
            assert_eq!(/*deref=*/*s_mut_ref2, "hello2");
            assert_eq!(s, "hello2");
        }
        println!("s = {:?}", s);
    }
    pub fn ref_as_raw_pointer() {
        let mut s = "hello".to_string();
        let s_ptr = &mut s as *mut String;
        unsafe { *s_ptr = "hello1".to_string() };
        assert_eq!(s, "hello1");
        
        // let s2 = *s_ptr;  // can not move out behind raw ptr.

        // ptr is primitive, copy, not move
        let s_ptr2 = s_ptr;
        unsafe { *s_ptr2 = "world".to_string() };
        // deref a raw ptr, value can not move out as it is behind raw ptr.
        assert_eq!(s, "world");
        assert_eq!(unsafe {& *s_ptr}, "world");
        assert_eq!(unsafe {& *s_ptr2}, "world");
        
        let mut s_ref = unsafe { &mut *s_ptr};
        s_ref.push('1');
        assert_eq!(s, "world1");
        assert_eq!(unsafe {& *s_ptr}, "world1");
        assert_eq!(unsafe {& *s_ptr2}, "world1");
    }

    pub fn cast_ptr() {
        let mut x = 10;
        let mut x_const_ptr = &x as *const i32;
        let mut x_mut_ptr = &mut x as *mut i32;
        assert_eq!(unsafe{*x_const_ptr}, 10);
        assert_eq!(unsafe{*x_mut_ptr}, 10);

        x_const_ptr = x_mut_ptr as *const i32;
        x_mut_ptr = x_const_ptr as *mut i32;
        assert_eq!(unsafe{*x_const_ptr}, 10);
        assert_eq!(unsafe{*x_mut_ptr}, 10);
    }
    // NonNull impls Copy Trait, ptr is copy
    pub fn nonnull() {
        let mut s = "hello".to_string();
        let mut ptr = NonNull::<String>::new(&mut s as *mut _).expect("non null is valid");
        
        let ptr2 = ptr; // no move
        assert_eq!(unsafe { &*ptr.as_ptr() }, "hello");
        assert_eq!(unsafe { &*ptr2.as_ptr() }, "hello");
        
        unsafe { *ptr2.as_ptr() = "world".to_string()};
        assert_eq!(s, "world");
        assert_eq!(unsafe { &*ptr.as_ptr() }, "world");
        assert_eq!(unsafe { &*ptr2.as_ptr() }, "world");
        
        let mut ref1 = unsafe { ptr.as_mut() };
        ref1.push('1');
        assert_eq!(s, "world1");
        assert_eq!(ref1, "world1");
        assert_eq!(unsafe { &*ptr.as_ptr() }, "world1");
        assert_eq!(unsafe { &*ptr2.as_ptr() }, "world1");
        
        let mut s2 = "hello".to_string();
        let mut s2_ptr = unsafe { NonNull::new_unchecked(&mut s2) };
        let s2_ref1 = &mut s2_ptr; // even copyable value is guarded by borrow rules.
        unsafe {*s2_ref1.as_mut() = "world".to_string(); }
        assert_eq!(unsafe{s2_ref1.as_ref()}, "world");

        // // NonNull<T> is copyable. Option<NonNull<T> also copyable.
        let mut cp1 = ptr;
        let mut cp2 = ptr;

        let mut cp1ref = unsafe { cp1.as_mut() };
        let mut cp2ref = unsafe { cp2.as_mut() };
        let cp1ref_ref = &mut cp1ref;
        // println!("cp1ref={:?}", cp1ref);  // already mut borrowed.
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
    borrow_reborrow::move_or_reborrow_mut_ref();
    borrow_reborrow::reborrow();
    borrow_reborrow::ref_as_pointer();
    borrow_reborrow::cast_ptr();
    borrow_reborrow::nonnull();
    borrow_reborrow::atomic_ptr();
    variance::test();
}
