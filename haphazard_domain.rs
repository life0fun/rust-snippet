//#![feature(arbitrary_self_types)]
#![deny(unsafe_op_in_unsafe_fn)]
#![allow(dead_code)]

use std::collections::HashSet;
use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};
use std::sync::atomic::Ordering;
use std::sync::atomic::{AtomicBool, AtomicPtr, AtomicUsize};
use std::sync::Arc;

// Writer wraps T into HazPtrObjectWrapper<T> and Box into AtomicPtr.
// T -> HazPtrObjectWrapper<T> -> Box::into_raw(Box::New) -> swap into AtomicPtr.
// AtomicPtr(Box::into_raw(Box::new(HazPtrObjWrapper(T))))
// HazPtrObjWrapper Deref to &T.
//
// HazPtrHolder has a domain lifetime ref to a HazPtr entry in the domain.
// Reader thread loads a AtomicPtr, protect the ptr with HazPtrHolder::hazptr entry for tracking.
// Holder ref to domain and acquires a ref to a hazptr from domain to hold client's ptr.
// load(&AtomicPtr<T>) protects the ptr behind the AtomicPtr<HazPtrObjectWrapper> into domain's HazPtr entry.
// When holder was droped, the HazPtr entry is de-actived and reset from active HazPtr list to be GCed.
//
// Reader loads the AtomicPtr<HazPtrObjWrapper<T>>, cast *mut p to Box ptr to HazPtrObjWrapper<T>
// Reader grabs a HazPtr entry from domain, protect the ptr into HazPtr entry.
// When reader done and drops the use of the ptr, HazPtr reset the protected ptr to null, disable.
// Domain claim find the HazPtr not active, put into retired list, and reclaim all non active HazPtr. 
// 
// HazPtrHolder::get_hazptr_from_domain() -> HazPtr and 
// HazPtrHolder::load(ptr) -> HazPtr::protect() return ptr to the original HazPtrObjectWrapper<T>
// &HazPtrObjectWrapper has Deref to &T.
// 

// when swap AtomicPtr<HazPtrObjectWrapper<T>>, the swaped out old value can not be reclaimed immediately,
// instead, T has to expose a retired API for us to put it into Domain's retired_list 
// and bulk claim later.
// When load AtomicPtr<T>, the T ptr must be linked into Domain's HazPtrs active list.
// When ptr is drop, its active flag in HazPtr is flase, hence can be bulk claimed later. 

// Domain { Retired, HazPtrs }
// HazPtrs { HazPtr< HazPtrObjectWrapper <T> >> }
// Retired { *mut dyn Reclaim, &dyn Deleter, next, ... }

#[non_exhaustive]
pub struct Global;
impl Global {
    const fn new() -> Self {
        Global
    }
}

// marker trait for all HazPtrObject, cast the swapped out 
// HazPtrObjectWrapper<T> self as *mut (dyn Reclaim + 'domain) to retire API.
pub trait Reclaim {}
impl<T> Reclaim for T {}

// mod FnItemTest {
    // trait Animal { fn speak(&self); }
    // struct Dog;
    // impl Animal for Dog {
    //     fn speak(&self) { println!("speak: dog"); }
    // }
    // struct Cat;
    // impl Animal for Cat {
    //     fn speak(&self) { println!("speak: cat"); }
    // }
    
    // fn add_one(x: i32) -> i32 { x + 1 }
    // impl Animal for fn(i32) -> i32 {
    //     fn speak(&self) { println!("speak: fn(i32) -> i32"); }
    // }
    // pub const ADD_PTR: fn(i32) -> i32 = add_one;
    
    // fn take_fn_ptr(f: fn(i32) -> i32, arg: i32) -> i32 {
    //     f(arg) + f(arg)
    // }
    // fn take_fn_ref(f: &fn(i32) -> i32, arg: i32) -> i32 {
    //     f(arg) + f(arg)
    // }
    // fn animal_speak(animal: &dyn Animal) {
    //     animal.speak();
    // }
    // fn main() {
    //     println!("take_fn_ptr: {}", take_fn_ptr(add_one, 5));
    //     //println!("take_fn_ref: {}", take_fn_ref(&add_one, 5));
    //     println!("take_fn_ref: {}", take_fn_ref(&ADD_PTR, 5));
    //     let d = Dog;
    //     animal_speak(&d);
    //     // animal_speak(&add_one);
    //     animal_speak(&ADD_PTR);
    // }
//}

pub trait Deleter {
    unsafe fn delete(&self, ptr: *mut dyn Reclaim);
}
// impls Deleter trait to fn pointer type that takes *mut dyn Reclaim as arg.
impl Deleter for unsafe fn(*mut (dyn Reclaim + 'static)) {
    unsafe fn delete(&self, ptr: *mut dyn Reclaim) {
        // self = fn(*mut dyn Reclaim), = deleter.drop_box
        unsafe {
            (*self)(ptr);
        }
    }
}
pub mod deleters {
    use super::Reclaim;
    // we impled Deleter trait for fn(*mut dyn Reclaim)
    pub fn _drop_in_place(ptr: *mut dyn Reclaim) {
        unsafe { std::ptr::drop_in_place(ptr) }; // ptr is *mut T
    }
    // explicit coerce fn item to fn ptr as retire API takes &dyn Deleter
    #[allow(non_upper_case_globals)]  
    pub const drop_in_place: unsafe fn(*mut dyn Reclaim) = _drop_in_place;
    
    // _drop_box is a function item, its own zero-sized type.
    pub fn _drop_box(ptr: *mut dyn Reclaim) {
        unsafe {
            let _ = Box::from_raw(ptr);
        }
    }
    #[allow(non_upper_case_globals)] 
    // explicit coerce fn item to fn ptr, so the Deleter trait that impled for fn ptr can be applied.
    pub const drop_box: unsafe fn(*mut dyn Reclaim) = _drop_box;
}

// wrap an AtomicPtr swapped out *mut T into a Retired boxed in the RetiredList in a domain.
pub struct Retired {
    ptr: *mut dyn Reclaim, // ptr is Box::into_raw of a HazPtrObject that impls Reclaim. must take exclusive for Box::from_raw()
    // deleter: &'static dyn Deleter, // takes trait object pointer &dyn Deleter object
    deleter: fn(*mut dyn Reclaim),  // a fn pointer that takes a mut pointer to trait Reclaim
    next: AtomicPtr<Retired>,  // *mut T=Retired from Box::into_raw
}
impl Retired {
    // _ domain provides 'domain lifetime to Retired.
    pub fn new<'domain, F>(
        _: &'domain HazPtrDomain<F>,
        ptr: *mut (dyn Reclaim + 'domain),
        // deleter: &'static dyn Deleter,
        deleter: fn(*mut dyn Reclaim),
    ) -> Self {
        Self {
            // re-interpret cast lifetime 'domain => 'static
            ptr: unsafe { std::mem::transmute::<_, *mut (dyn Reclaim + 'static)>(ptr) },
            deleter,
            next: AtomicPtr::new(std::ptr::null_mut()), // AtomicPtr take *mut T;
        }
    }
}
pub struct RetiredList {
    head: AtomicPtr<Retired>,  // linked list to *mut Retired
    count: AtomicUsize,
}

// Reader stores loaded AtomicPtr *mut T into HazPtr, reclaim until all readers drops ref to ptr.
pub struct HazPtr {
    ptr: AtomicPtr<u8>, // user's ptr from Box::into_raw, cast to a different *mut T
    // next: Option<NonNull<HazPtr>>, // need a lock to serialize racing set next.
    next: AtomicPtr<HazPtr>, // lock-free, CAS to a next entry in the list
    active: AtomicBool,
}
impl HazPtr {
    fn protect(&self, user_t_ptr: *mut u8) {
        // stashing user's loaded AtomicPtr into HazPtr for later mem reclaim. 
        self.ptr.store(user_t_ptr, Ordering::SeqCst);
    }
}
// linked list head bucket or tree root of the collection.
pub struct HazPtrs {
    // head: Option<NonNull<HazPtr>>, // need a lock to serialze racing set the head.
    head: AtomicPtr<HazPtr>,
}
// Domain contain active HazPtrs and Retired user_T_ptrs.
pub struct HazPtrDomain<F> {
    hazptrs: HazPtrs,
    retired: RetiredList,
    family: PhantomData<F>, // compiler pls treats HazPtrDomain has this type.
}
static SHARED_DOMAIN: HazPtrDomain<Global> = HazPtrDomain::new(&Global::new());
// specialization of Global domain.
impl HazPtrDomain<Global> {
    pub fn global() -> &'static Self {
        &SHARED_DOMAIN
    }
}
impl<F> HazPtrDomain<F> {
    pub const fn new(_: &F) -> Self {
        // const fn can only take ref to avoid call destructors.
        Self {
            hazptrs: HazPtrs {
                // Note AtomicPtr::new takes mut raw ptr(*mut T)
                head: AtomicPtr::new(std::ptr::null_mut()),
            },
            retired: RetiredList {
                head: AtomicPtr::new(std::ptr::null_mut()),
                count: AtomicUsize::new(0),
            },
            family: PhantomData,
        }
    }
    // Safety: HazPtrs are never de-allocated.
    // return the shared ref to the hazptr in the hazptrs list in domain.
    // cas set active flag and other fields in HazPtr are all atomic values, hence &HazPtr.
    pub fn acquire(&self) -> &HazPtr {
        let head = &self.hazptrs.head;
        let mut cur_ptr = head.load(Ordering::SeqCst); // using shared ref atomicptr to load raw ptr, *mut T,
        loop {
            // following head next to find an non-active entry
            while !cur_ptr.is_null() && unsafe { &*cur_ptr }.active.load(Ordering::SeqCst) {
                cur_ptr = unsafe { &*cur_ptr }.next.load(Ordering::SeqCst);
            }
            if cur_ptr.is_null() {
                // No free HazPtrs -- need to allocate a new one
                let new_hazptr = Box::into_raw(Box::new(HazPtr {
                    ptr: AtomicPtr::new(std::ptr::null_mut()),
                    next: AtomicPtr::new(std::ptr::null_mut()),
                    active: AtomicBool::new(true),
                }));
                break loop {
                    // only use *get_mut() to update next() from null_mut to own old_head.
                    // otherwise, the existing *T in the AtomicPtr<T> will be leaking. 
                    // stick newly allocated as new head.
                    let mut old_head_ptr = head.load(Ordering::SeqCst);
                    *unsafe { &mut *new_hazptr }.next.get_mut() = old_head_ptr;
                    match head.compare_exchange_weak(
                        old_head_ptr,
                        new_hazptr,
                        Ordering::SeqCst,
                        Ordering::SeqCst,
                    ) {
                        Ok(_) => { break unsafe { &*new_hazptr }; }
                        Err(head_now) => { old_head_ptr = head_now; }
                    }
                };
            } else {
                let cur = unsafe { &*cur_ptr };
                if cur
                    .active
                    .compare_exchange_weak(false, true, Ordering::SeqCst, Ordering::SeqCst)
                    .is_ok()
                {
                    break cur;
                } // It's ours!
                  // keep walking as someone else grabbed this node racing.
            }
        }
    }
    pub(crate) unsafe fn retire<'domain>(
        &'domain self,  
        user_t_ptr: *mut (dyn Reclaim + 'domain),  // raw mut user_t_ptr to HazPtrObj<T> from Box::into_raw to retire
        // deleter: &'static dyn Deleter,
        deleter: fn(*mut dyn Reclaim),
    ) {
        // Box a Retired::new and link it to domain retired linked list AtomicPtr<Retired>.
        let newly_retired = Box::into_raw(Box::new(unsafe { Retired::new(self, user_t_ptr, deleter) }));
        self.retired.count.fetch_add(1, Ordering::SeqCst);
        let mut retired_head_ptr = self.retired.head.load(Ordering::SeqCst);
        loop {
            // *unsafe { &mut *newly_retired }.next.get_mut() = retired_head_ptr;
            unsafe {&mut *newly_retired }.next.store(retired_head_ptr, Ordering::SeqCst);
            match self.retired.head.compare_exchange_weak(
                retired_head_ptr,
                newly_retired,
                Ordering::SeqCst,
                Ordering::SeqCst,
            ) {
                Ok(_) => {
                    break;
                }
                Err(now_head_ptr) => {
                    retired_head_ptr = now_head_ptr;
                }
            }
        }
        if self.retired.count.load(Ordering::SeqCst) != 0 {
            self.bulk_reclaim(0, false);
        }
    }
    pub fn eager_reclaim(&self, block: bool) -> usize {
        self.bulk_reclaim(0, block)
    }
    fn active_hazptrs(&self) -> HashSet<*mut u8> {
        let mut active_ptrs = HashSet::new();
        // start with cur <= head; while !cur.is_null(); fn(cur); cur=cur.next;
        let mut hazptr_ptr = self.hazptrs.head.load(Ordering::SeqCst);
        while !hazptr_ptr.is_null() {
            let hazptr = unsafe { &*hazptr_ptr };
            if hazptr.active.load(Ordering::SeqCst) {
                active_ptrs.insert(hazptr.ptr.load(Ordering::SeqCst));  // ptr value(u8) to T into hashset.
            }
            hazptr_ptr = hazptr.next.load(Ordering::SeqCst);
        }
        active_ptrs
    }
    fn bulk_reclaim(&self, prev_reclaimed: usize, block: bool) -> usize {
        let retired_list_head = self
            .retired
            .head
            .swap(std::ptr::null_mut(), Ordering::SeqCst);
        if retired_list_head.is_null() {
            return 0;
        }

        let active_ptrs = self.active_hazptrs();

        // Reclaim any retired objects that aren't guarded
        let mut still_guarded_head_ptr: *mut Retired = std::ptr::null_mut(); //
        let mut tail = None;
        let mut reclaimed: usize = 0;
        // walk the retired list
        let mut cur_ptr = retired_list_head;
        while !cur_ptr.is_null() {
            let cur_retired = unsafe { &*cur_ptr };  // cast *mut T to &T. 
            let mut next_retired_ptr = cur_retired.next.load(Ordering::SeqCst);
            if active_ptrs.contains(&(cur_retired.ptr as *mut u8)) {
                // still guarded, not safe to reclaim, insert as the head of still_guarded_list
                cur_retired
                    .next
                    .store(still_guarded_head_ptr, Ordering::SeqCst);
                still_guarded_head_ptr = cur_ptr; // *mut T is plain-old-obj, no move
                if tail.is_none() {
                    tail = Some(still_guarded_head_ptr);
                }
            } else {
                // Retired is in heap by Box::new(), back to box to own the ptr, and delete the box also.
                let boxed_retired_node = unsafe { Box::from_raw(cur_ptr) }; // box own the cur_ptr
                // unsafe { boxed_retired_node.deleter.delete(boxed_retired_node.ptr) };
                unsafe { (boxed_retired_node.deleter)(boxed_retired_node.ptr) };
                reclaimed += 1;
                // drop the retired box that owns the ptr
            }
            cur_ptr = next_retired_ptr;
        }

        self.retired.count.fetch_sub(reclaimed, Ordering::SeqCst);
        let total_reclaimed = prev_reclaimed + reclaimed;

        // stick back still_guarded list
        let tail_ptr = if let Some(tail_ptr) = tail {
            assert!(!still_guarded_head_ptr.is_null());
            tail_ptr
        } else {
            return total_reclaimed;
        };

        let mut retired_list_head_ptr = self.retired.head.load(Ordering::SeqCst);
        let still_guarded_tail = unsafe { &mut *tail_ptr };
        loop {
            *unsafe { still_guarded_tail.next.get_mut() } = retired_list_head_ptr;
            match self.retired.head.compare_exchange_weak(
                retired_list_head_ptr,
                still_guarded_head_ptr,
                Ordering::SeqCst,
                Ordering::SeqCst,
            ) {
                Ok(_) => {
                    break;
                }
                Err(now_retired_list_head_ptr) => {
                    retired_list_head_ptr = now_retired_list_head_ptr;
                }
            }
        }
        total_reclaimed
    }
}

impl<F> Drop for HazPtrDomain<F> {
    fn drop(&mut self) {
        let nretired = *self.retired.count.get_mut();
        let nreclaimed = self.bulk_reclaim(0, false);
        assert_eq!(nretired, nreclaimed);
        assert!(self.retired.head.get_mut().is_null());
        // drop all hazptrs
        let mut hazptr_ptr: *mut HazPtr = *self.hazptrs.head.get_mut();
        while !hazptr_ptr.is_null() {
            let mut hazptr: Box<HazPtr> = unsafe { Box::from_raw(hazptr_ptr) };
            assert!(*hazptr.active.get_mut());
            hazptr_ptr = *hazptr.next.get_mut();
            drop(hazptr);
        }
    }
}

//
// public APIs and structs, HazPtrObjectWrapper and HazPtrHolder.
//

// AtomicPtr<HazPtrObjectWrapper<T>> impl this trait to expose retire API 
// link swapped out ptr into domain retired linklist and reclaim when all readers done.
pub trait HazPtrObject<'domain, F: 'static>
where
    Self: Sized + 'domain,
{
    fn domain(&self) -> &'domain HazPtrDomain<F>;
    // deleter is dyn dispatch trait pointer.
    // unsafe fn retire(&mut self, deleter: &'static dyn Deleter) {
    unsafe fn retire(&mut self, deleter: fn(*mut dyn Reclaim)) {
        // cast HazPtrObject to dyn Reclaim ptr
        let ptr = self as *mut (dyn Reclaim + 'domain);
        unsafe {
            (&*self).domain().retire(ptr, deleter);
        }
    }
}
// Wrap T ptr, expose retire API that delegates to domain's retire() where list of retired ptr bulk claimed.
// AtomicPtr::new(...(Box::new(Wrapper::with_domain(T)))
pub struct HazPtrObjectWrapper<'domain, T, F> {
    inner: T,
    domain: &'domain HazPtrDomain<F>,
}

impl<T> HazPtrObjectWrapper<'static, T, Global> {
    pub fn with_global_domain(t: T) -> Self {
        HazPtrObjectWrapper::with_domain(HazPtrDomain::global(), t)
    }
}
impl<'domain, T, F> HazPtrObjectWrapper<'domain, T, F> {
    pub fn with_domain(domain: &'domain HazPtrDomain<F>, t: T) -> Self {
        Self { inner: t, domain }
    }
}

impl<'domain, T: 'domain, F: 'static> HazPtrObject<'domain, F>
    for HazPtrObjectWrapper<'domain, T, F>
{
    fn domain(&self) -> &'domain HazPtrDomain<F> {
        self.domain
    }
}
impl<T, F> Deref for HazPtrObjectWrapper<'_, T, F> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl<T, F> DerefMut for HazPtrObjectWrapper<'_, T, F> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

//
// Reader thread uses HazPtrHolder load to read AtomicPtr *mut u8, protect it into HazPtrHolder::hazptr
// so it can be Gced after drop.
// Holder ref to domain and acquires a ref to a hazptr from domain to hold client's ptr.
// load(AtomicPtr) stores a client's AtomicPtr<HazPtrObjectWrapper> into domain's HazPtr entry.
// so client's AtomicPtr can be protected and GCed by domain.
//
// T -> HazPtrObjectWrapper<T> -> Box::into_raw(Box::New) -> swap into AtomicPtr.
// AtomicPtr(Box::into_raw(Box::new(HazPtrObjWrapper(T))))
// HazPtrObjWrapper Deref to &T.
//
// Reader loads the AtomicPtr<HazPtrObjWrapper<T>>, cast *mut p to Box ptr to HazPtrObjWrapper<T>
// Reader grabs a HazPtr entry from domain, protect the ptr into HazPtr entry.
// When reader done and drops the use of the ptr, HazPtr reset the protected ptr to null, disable.
// Domain claim find the HazPtr not active, put into retired list, and reclaim all non active HazPtr. 
// 
// HazPtrHolder::get_hazptr_from_domain() -> HazPtr and 
// HazPtrHolder::load(ptr) -> HazPtr::protect() return ptr to the original HazPtrObjectWrapper<T>
// &HazPtrObjectWrapper has Deref to &T.
struct HazPtrHolder<'domain, F> {
    hazptr: Option<&'domain HazPtr>,  // a ref to HazPtr in the domain
    domain: &'domain HazPtrDomain<F>, // holder belongs to a domain
}
// Obtain a HazPtrHolder, which only has two refs to domain and HazPtr entry in domain.
impl HazPtrHolder<'static, crate::Global> {
    fn global() -> Self {
        HazPtrHolder::for_domain(HazPtrDomain::global())
    }
}
impl<'domain, F> HazPtrHolder<'domain, F> {
    fn for_domain(domain: &'domain HazPtrDomain<F>) -> Self {
        Self {
            hazptr: None,
            domain,
        }
    }
    // get a hazptr entry from domain hazptrs list.
    fn get_hazptr_from_domain(&mut self) -> &'domain HazPtr {
        if let Some(hazptr) = self.hazptr {
            hazptr
        } else {
            let hazptr = self.domain.acquire(); // a ref to an entry domain HazPtr list
            self.hazptr = Some(hazptr);
            hazptr
        }
    }
    // Reader uses HazPtrHolder load(&AtomicPtr<HazPtrObjWrap<T>>) to read *mut u8.
    // the data ptr behind AtomicPtr is then protected into HazPtrHolder and tracked for reclaim.
    pub unsafe fn load<'l, 'o, T>(&'l mut self, atomic_ptr: &'_ AtomicPtr<T>) -> Option<&'l T>
    where
        T: HazPtrObject<'o, F>, // HazPtrObjectWrapper<'o, >
        'o: 'l,
        F: 'static,
    {
        let hazptr = self.get_hazptr_from_domain();
        let mut user_t_ptr = atomic_ptr.load(Ordering::SeqCst);
        loop {
            hazptr.protect(user_t_ptr as *mut u8); // store atomic_ptr
            let ptr_now = atomic_ptr.load(Ordering::SeqCst);
            if user_t_ptr == ptr_now {
                // no change of user data atomicptr
                // we can do unsafe { ptr.as_ref() }; but check not null by NonNull
                break std::ptr::NonNull::new(user_t_ptr).map(|nonnull_ptr| {
                    unsafe { nonnull_ptr.as_ref() } // map *mut T to &T for return
                });
            } else {
                user_t_ptr = ptr_now; // assign the latest ptr for hazptr to protect.
            }
        }
    }
    pub fn reset(&mut self) {
        if let Some(hazptr) = self.hazptr {
            hazptr.ptr.store(std::ptr::null_mut(), Ordering::SeqCst);
        }
    }
}
// Drop a HazPtrHolder never delete the holder, just de-active and reset the hazptr.
impl<F> Drop for HazPtrHolder<'_, F> {
    fn drop(&mut self) {
        self.reset();
        if let Some(hazptr) = self.hazptr {
            hazptr.active.store(false, Ordering::SeqCst);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct CountDrops(Arc<AtomicUsize>);
    impl Drop for CountDrops {
        fn drop(&mut self) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
    }

    #[test]
    fn feels_good() {
        let drops_42 = Arc::new(AtomicUsize::new(0));

        let x = AtomicPtr::new(Box::into_raw(Box::new(
            HazPtrObjectWrapper::with_global_domain((42, CountDrops(Arc::clone(&drops_42)))),
        )));

        // As a reader, you shall always grab a holder to protect the AtomicPtr you are using by loading it to holder.
        let mut h = HazPtrHolder::global();

        // Safety:
        //
        //  1. AtomicPtr points to a Box, so is always valid.
        //  2. Writers to AtomicPtr use HazPtrObject::retire.
        let my_x = unsafe { h.load(&x) }.expect("not null");
        // let reader2_x = unsafe { h.load(&x) }.expect("not null");
        // valid:
        assert_eq!(my_x.0, 42);
        h.reset();
        // invalid:
        // let _: i32 = my_x.0;

        let my_x = unsafe { h.load(&x) }.expect("not null");
        // valid:
        assert_eq!(my_x.0, 42); // HazPtrObjectWrapper<'o, ({42, CountDrops})>
        drop(h);
        // invalid:
        // let _: i32 = my_x.0;

        let mut h = HazPtrHolder::global();
        let my_x = unsafe { h.load(&x) }.expect("not null");

        let mut h_tmp = HazPtrHolder::global();
        let _ = unsafe { h_tmp.load(&x) }.expect("not null");
        drop(h_tmp);

        // As a writer:
        let drops_9001 = Arc::new(AtomicUsize::new(0));
        let old = x.swap(
            Box::into_raw(Box::new(HazPtrObjectWrapper::with_global_domain((
                9001,
                CountDrops(Arc::clone(&drops_9001)),
            )))),
            std::sync::atomic::Ordering::SeqCst,
        );

        let mut h2 = HazPtrHolder::global();
        let my_x2 = unsafe { h2.load(&x) }.expect("not null");

        assert_eq!(my_x.0, 42);
        assert_eq!(my_x2.0, 9001);

        // Safety:
        //
        //  1. The pointer came from Box, so is valid.
        //  2. The old value is no longer accessible.
        //  3. The deleter is valid for Box types.
        // unsafe { { &mut *old }.retire(&deleters::drop_box) };
        unsafe { { &mut *old }.retire(deleters::_drop_box) };

        assert_eq!(drops_42.load(Ordering::SeqCst), 0);
        assert_eq!(my_x.0, 42);

        let n = HazPtrDomain::global().eager_reclaim(false);
        assert_eq!(n, 0);

        assert_eq!(drops_42.load(Ordering::SeqCst), 0);
        assert_eq!(my_x.0, 42);

        drop(h);
        assert_eq!(drops_42.load(Ordering::SeqCst), 0);
        // _not_ drop(h2);

        let n = HazPtrDomain::global().eager_reclaim(false);
        assert_eq!(n, 1);

        assert_eq!(drops_42.load(Ordering::SeqCst), 1);
        assert_eq!(drops_9001.load(Ordering::SeqCst), 0);

        drop(h2);
        let n = HazPtrDomain::global().eager_reclaim(false);
        assert_eq!(n, 0);
        assert_eq!(drops_9001.load(Ordering::SeqCst), 0);
    }

    #[test]
    #[should_panic]
    fn feels_bad() {
        let dw = HazPtrDomain::new(&());
        let dr = HazPtrDomain::new(&());

        let drops_42 = Arc::new(AtomicUsize::new(0));

        let x = AtomicPtr::new(Box::into_raw(Box::new(HazPtrObjectWrapper::with_domain(
            &dw,
            (42, CountDrops(Arc::clone(&drops_42))),
        ))));

        // Reader uses a different domain thant the writer!
        let mut h = HazPtrHolder::for_domain(&dr);

        // Let's hope this catches the error (at least in debug mode).
        let _ = unsafe { h.load(&x) }.expect("not null");
    }
}
