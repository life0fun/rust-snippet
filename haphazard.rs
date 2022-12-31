//#![feature(arbitrary_self_types)]
#![deny(unsafe_op_in_unsafe_fn)]
#![allow(dead_code)]

use std::collections::HashSet;
use std::marker::PhantomData;
use std::sync::atomic::Ordering;
use std::sync::atomic::{AtomicBool, AtomicPtr, AtomicUsize};
use std::ops::{Deref, DerefMut};
use std::sync::Arc;

// Domain { Retired, HazPtrs } 
// HazPtrs { HazPtr< HazPtrObjectWrapper <T> >> }
// Retired { *mut dyn Reclaim, &dyn Deleter, next, ... }


#[non_exhaustive]
pub struct Global;
impl Global {
    const fn new() -> Self { Global }
}

// marker trait so 
pub trait Reclaim {}
impl<T> Reclaim for T {}
// delete a ptr, any ptr, as all T impls Reclaim.
pub trait Deleter {
    unsafe fn delete(&self, ptr: *mut dyn Reclaim);
}

pub struct HazPtr {
    ptr: AtomicPtr<u8>,      // cast to a different *mut T
    // next: Option<NonNull<HazPtr>>, // need a lock to serialize racing set next.
    next: AtomicPtr<HazPtr>, // lock-free, CAS to a next entry in the list
    active: AtomicBool,
}
impl HazPtr {
    fn protect(&self, ptr: *mut u8) { // take a *mut, store it into self HazPtr entry's AtomicPtr
        self.ptr.store(ptr, Ordering::SeqCst);
    }
}
// linked list head bucket or tree root of the collection.
pub struct HazPtrs {
    // head: Option<NonNull<HazPtr>>, // need a lock to serialze racing set the head.
    head: AtomicPtr<HazPtr>,
}
// wrap a reclaimable ptr into a Retired entry in the RetiredList head.
pub struct Retired {
    ptr: *mut dyn Reclaim, // expose only Reclaim trait of the object pointed.
    deleter: &'static dyn Deleter,  // trait object fat pointer.
    next: AtomicPtr<Retired>,
}
impl Retired { // _ domain provides 'domain lifetime to Retired.
    pub fn new<'domain, F>(_: &'domain HazPtrDomain<F>, ptr: *mut (dyn Reclaim + 'domain), 
        deleter: &'static dyn Deleter) -> Self {
        Self { // re-interpret cast to cast lifetime 'domain to 'static
            ptr: unsafe { std::mem::transmute::<_, *mut (dyn Reclaim + 'static)>(ptr) },
            deleter,
            next: AtomicPtr::new(std::ptr::null_mut()),  // AtomicPtr with null_mut
        }
    }
}

pub struct RetiredList {
    head: AtomicPtr<Retired>,
    count: AtomicUsize,
}
pub struct HazPtrDomain<F> {
    hazptrs: HazPtrs,
    retired: RetiredList,
    family: PhantomData<F>,  // compiler pls treats HazPtrDomain has this type.
}
static SHARED_DOMAIN: HazPtrDomain<Global> = HazPtrDomain::new(&Global::new());
// specialization of Global domain.
impl HazPtrDomain<Global> {
    pub fn global() -> &'static Self { &SHARED_DOMAIN }
}
impl<F> HazPtrDomain<F> {
    pub const fn new(_: &F) -> Self { // const fn can only take ref to avoid call destructors.
        Self{
            hazptrs: HazPtrs {  // Note AtomicPtr::new takes mut raw ptr(*mut T)
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
    pub fn acquire(&self) -> &HazPtr {  // ref to an inactive HazPtr entry in domain. 
        let head = &self.hazptrs.head;
        let mut cur_ptr = head.load(Ordering::SeqCst); // load ret raw ptr, *mut T, 
        loop {  // init/loop/update
            while !cur_ptr.is_null() && unsafe { &*cur_ptr }.active.load(Ordering::SeqCst) {
                // loop (if cur active), set cur to the next entry in list.
                cur_ptr = unsafe { &*cur_ptr }.next.load(Ordering::SeqCst);
            }
            if cur_ptr.is_null() {
                // No free HazPtrs -- need to allocate a new one
                let new_head_ptr = Box::into_raw(Box::new(HazPtr {
                    ptr: AtomicPtr::new(std::ptr::null_mut()),
                    next: AtomicPtr::new(std::ptr::null_mut()),
                    active: AtomicBool::new(true),
                }));
                // init old / loop stick new head to head / update old head
                let mut old_head_ptr = head.load(Ordering::SeqCst);
                break loop {
                    // mutate the inner pointer directly via get_mut();
                    *unsafe { &mut *new_head_ptr }.next.get_mut() = old_head_ptr;
                    match head.compare_exchange_weak(
                        old_head_ptr,
                        new_head_ptr,
                        Ordering::SeqCst,
                        Ordering::SeqCst,
                    ) {
                        Ok(_) => { break unsafe { &*new_head_ptr }; }
                        Err(head_now) => { old_head_ptr = head_now; }
                    }
                };
            } else {
                let cur = unsafe { &*cur_ptr };
                if cur.active.compare_exchange_weak(false, true, Ordering::SeqCst, Ordering::SeqCst)
                    .is_ok()
                { break cur; } // It's ours!
                // keep walking as someone else grabbed this node racing.
            }
        }
    }
    pub(crate) unsafe fn retire<'domain>(
        &'domain self, ptr: *mut (dyn Reclaim + 'domain), deleter: &'static dyn Deleter) {
        // A Box new Retired to wrap the reclaimed ptr and its deleter.  
        let newly_retired = Box::into_raw(Box::new(unsafe { Retired::new(self, ptr, deleter) } ));
        self.retired.count.fetch_add(1, Ordering::SeqCst);
        let mut retired_head_ptr = self.retired.head.load(Ordering::SeqCst);
        loop {
            *unsafe { &mut *newly_retired }.next.get_mut() = retired_head_ptr;
            match self.retired.head.compare_exchange_weak(retired_head_ptr, newly_retired, Ordering::SeqCst, Ordering::SeqCst) {
                Ok(_) => { break; }
                Err(now_head_ptr) => { retired_head_ptr = now_head_ptr; }
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
        let mut hazptr_ptr = self.hazptrs.head.load(Ordering::SeqCst);
        while !hazptr_ptr.is_null() {
            let hazptr = unsafe { &* hazptr_ptr };
            if hazptr.active.load(Ordering::SeqCst) {
                active_ptrs.insert(hazptr.ptr.load(Ordering::SeqCst));
            }
            hazptr_ptr = hazptr.next.load(Ordering::SeqCst);
        }
        active_ptrs
    }
    fn bulk_reclaim(&self, prev_reclaimed: usize, block: bool) -> usize {
        let retired_list_ptr = self.retired.head.swap(std::ptr::null_mut(), Ordering::SeqCst);
        if retired_list_ptr.is_null() { return 0; }

        let active_ptrs = self.active_hazptrs();

        // Reclaim any retired objects that aren't guarded
        let mut still_guarded_head_ptr: *mut Retired = std::ptr::null_mut();  // 
        let mut tail = None;
        let mut reclaimed: usize = 0;
        let mut cur_ptr = retired_list_ptr;
        while !cur_ptr.is_null() {
            // let cur_retired_ptr = retired_ptr;
            let cur_retired = unsafe { &* cur_ptr };
            if active_ptrs.contains(&(cur_retired.ptr as *mut u8)) {
                // still guarded, not safe to reclaim, link back
                cur_retired.next.store(still_guarded_head_ptr, Ordering::SeqCst);
                still_guarded_head_ptr = cur_ptr;  // *mut T is plain-old-obj, no move
                if tail.is_none() {
                    tail = Some(still_guarded_head_ptr);
                } 
            } else { // Retired is in heap by Box::new(), back to box to own the ptr, and delete the box also.
                let retired_box = unsafe { Box::from_raw(cur_ptr) };  // box own the cur_ptr
                unsafe { retired_box.deleter.delete(retired_box.ptr) };
                reclaimed += 1;
                // drop the retired box that owns the ptr
            }
             cur_ptr = cur_retired.next.load(Ordering::SeqCst);
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
        
        let mut cur_head_ptr = self.retired.head.load(Ordering::SeqCst);
        let still_guarded_tail = unsafe { &mut *tail_ptr };
        loop {
            *unsafe { still_guarded_tail.next.get_mut() } = cur_head_ptr;
            match self.retired.head.compare_exchange_weak(
                cur_head_ptr, still_guarded_head_ptr, Ordering::SeqCst, Ordering::SeqCst
            ) {
                Ok(_) => { break; }
                Err(now_head_ptr) => {
                    cur_head_ptr = now_head_ptr;
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

// impls Deleter trait to all fn pointers that drops ptr: *mut dyn Reclaim.
impl Deleter for unsafe fn(*mut (dyn Reclaim + 'static)) {
    unsafe fn delete(&self, ptr: *mut dyn Reclaim) {
        // self = fn(*mut dyn Reclaim), = deleter.drop_box
        unsafe { (*self)(ptr); }
    }
}
// fn pointer that drops *mut dyn Reclaim.
pub mod deleters {
    use super::Reclaim;
    unsafe fn _drop_in_place(ptr: *mut dyn Reclaim) {
        unsafe { std::ptr::drop_in_place(ptr) };
    }
    #[allow(non_upper_case_globals)]
    pub const drop_in_place: unsafe fn(*mut dyn Reclaim) = _drop_in_place;
        
    unsafe fn _drop_box(ptr: *mut dyn Reclaim) {
        unsafe { let _ = Box::from_raw(ptr); }
    }
    #[allow(non_upper_case_globals)]
    pub const drop_box: unsafe fn(*mut dyn Reclaim) = _drop_box;
}

// trait exposes domain and retire methods to all value wrapped by HazPtrObjectWrapper.
pub trait HazPtrObject<'domain, F: 'static>
where 
    Self: Sized + 'domain
{
    fn domain(&self) -> &'domain HazPtrDomain<F>;
    // cast HazPtrObject to ptr to dyn Reclaim and use domain's retire to retire ptr.
    unsafe fn retire(&mut self, deleter: &'static dyn Deleter) {
        let ptr = self as *mut (dyn Reclaim + 'domain);
        unsafe { (&*self).domain().retire(ptr, deleter); }
    }    
}
// composite by wrapping vs. inherite ObjectBase ?
// embeds any T value and gives out T' that impls HazPtrObject Trait to store in HazPtr.
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

impl<'domain, T: 'domain, F: 'static> HazPtrObject<'domain, F> for 
HazPtrObjectWrapper<'domain, T, F> {
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

// T -> HazPtrObjectWrapper<T> -> Box::into_raw(Box::New) -> AtomicPtr.
// AtomicPtr(Box::into_raw(Box::new(HazPtrObjWrapper(T))))
// 
// holder acquires and hold a ref to domain's hazptrs HazPtr entry.
// load(AtomicPtr) stores a AtomicPtr<HazPtrObjectWrapper> to the HazPtr entry
// so it will be protected and GCed by domain.
//
struct HazPtrHolder<'domain, F> {
    hazptr: Option<&'domain HazPtr>,
    domain: &'domain HazPtrDomain<F>,   // holder belongs to a domain
}

impl HazPtrHolder<'static, crate::Global> {
    fn global() -> Self {
        HazPtrHolder::for_domain(HazPtrDomain::global())
    }
}
impl<'domain, F> HazPtrHolder<'domain, F> {
    fn for_domain(domain: &'domain HazPtrDomain<F>) -> Self {
        Self { hazptr: None, domain }
    }
    // get a hazptr entry from domain hazptrs list.
    fn get_hazptr_from_domain(&mut self) -> &'domain HazPtr {
        if let Some(hazptr) = self.hazptr {
            hazptr
        } else {
            let hazptr = self.domain.acquire();  // a ref to an entry domain HazPtr list
            self.hazptr = Some(hazptr);
            hazptr
        }
    }
    // Given a mut ref to user's AtomicPtr and store its inner raw ptr into the holder's hazptr's AtomicPtr
    pub unsafe fn load<'l, 'o, T>(&'l mut self, user_data_atomicptr: &'_ AtomicPtr<T>) -> Option<&'l T> 
    where
        T: HazPtrObject<'o, F>,  // HazPtrObjectWrapper<'o, >
        'o: 'l,
        F: 'static
    {
        let domain_hazptr = self.get_hazptr_from_domain();
        let mut user_data_ptr = user_data_atomicptr.load(Ordering::SeqCst);
        loop {
            domain_hazptr.protect(user_data_ptr as *mut u8);  // store atomic_ptr
            let ptr_now = user_data_atomicptr.load(Ordering::SeqCst);
            if user_data_ptr == ptr_now { // no change of user data atomicptr 
                // we can do unsafe { ptr.as_ref() }; but check not null by NonNull 
                break std::ptr::NonNull::new(user_data_ptr).map(|raw_ptr| {
                    unsafe { raw_ptr.as_ref() }  // map *mut T to &T for return
                });
            } else {
                user_data_ptr = ptr_now;  // assign the latest ptr for hazptr to protect.
            }
        }
    }
    pub fn reset(&mut self) {
        if let Some(hazptr) = self.hazptr {
            hazptr.ptr.store(std::ptr::null_mut(), Ordering::SeqCst);
        }
    }
}
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
    
        // As a reader:
        let mut h = HazPtrHolder::global();
    
        // Safety:
        //
        //  1. AtomicPtr points to a Box, so is always valid.
        //  2. Writers to AtomicPtr use HazPtrObject::retire.
        let my_x = unsafe { h.load(&x) }.expect("not null");
        // valid:
        assert_eq!(my_x.0, 42);
        h.reset();
        // invalid:
        // let _: i32 = my_x.0;
    
        let my_x = unsafe { h.load(&x) }.expect("not null");
        // valid:
        assert_eq!(my_x.0, 42);  // HazPtrObjectWrapper<'o, ({42, CountDrops})>
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
        unsafe { { &mut *old }.retire(&deleters::drop_box) };
    
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
