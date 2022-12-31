fn double(i: i32) -> i32 {
    i*2
  }
  
  fn static_dispatch(a: i32, multi: impl Fn(i32) -> i32) -> i32 {
    multi(a)
  }
  fn static_dispatch_ref(a: i32, multi: &impl Fn(i32) -> i32) -> i32 {
    multi(a)
  }
  
  fn dynamic_dispatch(a: i32, multi: &dyn Fn(i32) -> i32) -> i32 {
    multi(a)
  }
  
  fn fn_pointer(a: i32, fn_ptr: fn(i32) -> i32) -> i32 {
    fn_ptr(a)
  }
  
  pub trait Foo {}
  impl Foo for fn(*mut dyn Drop) {}
  pub fn bar(_: &dyn Foo) {}
  
  pub trait Foo2 {}
  impl Foo2 for fn() {}
  pub fn takes_foo2(f: &dyn Foo2) {}
  
  trait Foo3 {
      fn foo3_fn(&mut self);
  }
  
  impl Foo3 for fn() {
      fn foo3_fn(&mut self) {
          (*self)()
      }
  }
  
  fn takes_foo3(_: impl Foo3) {}
  fn is_foo3() {}
  
  // address of function pointer, fn, does not work!
  // fn fn_pointer_ref(a: i32, fn_ptr: &fn(i32) -> i32) -> i32 {
  //   (*fn_ptr)(a)
  // }
  
  #[cfg(test)]
  mod tests {
    use super::*;
    
    #[test]
    fn test() {
      assert_eq!(static_dispatch(4, double), 8);
      assert_eq!(static_dispatch(5, |x| x*2), 10);
  
      assert_eq!(static_dispatch_ref(4, &double), 8);
      assert_eq!(static_dispatch_ref(5, &|x| x*2), 10);
      
      assert_eq!(dynamic_dispatch(4, &double), 8);    
      assert_eq!(dynamic_dispatch(5, &|x| x*2), 10);
  
      
      assert_eq!(fn_pointer(4, double), 8);    
      assert_eq!(fn_pointer(5, |x| x*2), 10);
  
      // assert_eq!(fn_pointer_ref(5, &|x| x*2), 10);
    }
  
    #[test]
    fn test_function_item_pointer() {
      fn good(_: *mut dyn Drop) {}
      // All good:
      bar(&(good as fn(*mut dyn Drop)));
      // No good:
      //bar(&good);
      
      fn strawman() {}
      takes_foo2(&(strawman as fn()));
      //takes_foo2(&strawman);
      
      //takes_foo3(is_foo3);
      takes_foo3(is_foo3 as fn());
  
      //takes_foo3(|| {});
      takes_foo3((|| {}) as fn());
    }
  }