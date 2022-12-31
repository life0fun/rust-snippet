use core::marker::PhantomData;
use std::ptr::NonNull;


#[derive(Debug)]
struct Foo<T> {
    name: T,
}
// impl<T> Drop for Foo<T> {
//     fn drop(&mut self) {
//         let _ = self.name;
//     }
// }
pub struct Boks<T> {
    // p: *mut T,
    p: std::ptr::NonNull<T>,
    _p: PhantomData<T>,
}
impl<T> Drop for Boks<T> {
    fn drop(&mut self) {
        unsafe { Box::from_raw(self.p.as_mut()) };
    }
}
impl<T> Boks<T> {
   fn new(t: T) -> Self {
       Boks { 
            // p: Box::into_raw(Box::new(t)),  
            p: unsafe { NonNull::new_unchecked(Box::into_raw(Box::new(t))) },
            _p: PhantomData,
        }
   }
}
impl<T> std::ops::Deref for Boks<T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        unsafe { &*self.p.as_ref() }
    }
}
impl<T> std::ops::DerefMut for Boks<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *self.p.as_mut() }
    }
}

// the Deserializer construct is covariance over T,
// but does not drop check on T.
struct Deserializer<T> {
    _d: PhantomData<fn() ->T>,
}
struct EmptyIterator<T> {
    _d: PhantomData<fn() ->T>,
}
impl<T> Iterator for EmptyIterator<T> {
    type Item = T;
    fn next(&mut self) -> Option<Self::Item> {
        None
    }
}

use std::fmt::Debug;
struct Oisann<T: Debug>(T);
// a type whose drop check will touch T.
impl<T: Debug> Drop for Oisann<T> {
    fn drop(&mut self) {
        println!("{:?}", self.0);
    }
}

fn main() {
    {
        let mut foo = Foo { name: "john".to_string() };
        let b1 = Box::new(&mut foo);
        let mut z = "xx".to_string();
        let o = Boks::new(Oisann(&mut z));
        // println!("z = {}", z);
    }
    {
        let s = String::from("hei");
        let mut box1 = Boks::new(&*s);
        let box2: Boks<&'static str> = Boks::new("world");
        box1 = box2;    
    }
    {
        let s = String::from("hei");
        let mut box1 = Box::new(&*s);
        let box2: Box<&'static str> = Box::new("world");
        box1 = box2;    
    }
    {
        let mut a = 42;
        let mut o = Some(Oisann(&mut a));    // hold write lock to a
        drop(o);
        // get read lock shall fail befre o is dropped.
        println!("a={}", a);
    }
    {
        let mut s = "hell".to_string();
        let a = unsafe { NonNull::new_unchecked(&mut s) };
        let mut b = a;
        let mut c = a;
        let d = unsafe { b.as_mut() };
        println!("s={:?}", d);
        s = "world".to_string();
        let e = unsafe { c.as_mut() };
        println!("s={:?}", d);
        println!("e={:?}", e);
    }
}
