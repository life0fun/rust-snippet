use std::ops::Index;
use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};
use std::sync::atomic::Ordering;
use std::sync::atomic::{AtomicBool, AtomicPtr, AtomicUsize};
use std::thread;

pub const RETRY_THRESHOLD: usize = 10;
pub struct ContentionMeasure(usize);
pub struct Contention;


pub trait CasDescriptor {
    fn execute(&self) -> Result<(), ()>;
}
// bound to Index trait with usize Idx and Output=D bounds to CasDescriptor.
pub trait CasDescriptors<D>: Index<usize, Output = D> 
where D: CasDescriptor,
{
    fn len(&self) -> usize;
}

// A normalized lockfree generates CASes from Input Ops to executor run. 
// wrap_up performed CASes into Output. With CasDescriptor Trait object.
pub trait NormalizedLockFree {
    type Input: Clone;
    type Output: Clone;
    type Cas: CasDescriptor;
    type Cases: CasDescriptors<Self::Cas> + Clone;
    
    fn generate(&self, op: &Self::Input, contention: &mut ContentionMeasure) -> Self::Cases;
    fn wrap_up(&self, executed: Result<(), usize>, performed: &Self::Cases, 
                //contention: &mut ContentionMeasure
            ) 
                -> Result<Self::Output, Contention>;
}

// All enum data come from associate types from NormalizedLockFree trait.
enum OperationState<LF: NormalizedLockFree> {
    PreCas,
    ExecuteCas(LF::Cases),
    PostCas(LF::Cases, Result<(),usize>),
    Completed(LF::Output),
}
struct OperationRecordBox<LF: NormalizedLockFree> {
    val: AtomicPtr<OperationRecord<LF>>,
}
struct OperationRecord<LF: NormalizedLockFree> {
    owner: std::thread::ThreadId,
    // each state enum encapsulates the arg/result  
    input: LF::Input,
    state: OperationState<LF>,
}

pub struct HelpQueue<LF> {
    _o: PhantomData<LF>,
}
impl<LF: NormalizedLockFree> HelpQueue<LF> {
    fn enqueue(&self, help: *const OperationRecordBox<LF>) {
        let _ = help;
    }
    fn peek(&self) -> Option<*const OperationRecordBox<LF>> {
        None
    }
    fn try_remove_front(&self, front: *const OperationRecordBox<LF>) -> Result<(), ()> {
        let _ = front;
        Ok(())
    }
}

// WF executor take a normalizedLF which generate CAS ops.
// WF executor runs the CASes with a help queue to simulate wait free.
// The help queue stores AtomicPtr to boxed intermediated OperationRecord.
pub struct WaitFreeSimulator<LF: NormalizedLockFree> {
    algorithm: LF,
    help: HelpQueue<LF>,
}
impl<LF: NormalizedLockFree> WaitFreeSimulator<LF> {
    pub fn cas_execute(&self, descriptors: &LF::Cases) -> Result<(), usize> {
        let len = descriptors.len();
        for i in 0..len {
            if descriptors[i].execute().is_err() {
                return Err(i);
            }
        }
        Ok(())
    }
    pub fn help_first(&self) {
        if let Some(help) = self.help.peek() {
            self.help_op(unsafe {&*help} );
        }
    }
    fn help_op(&self, orb: &OperationRecordBox<LF>) {
        loop {
            let or = unsafe { &mut *orb.val.load(Ordering::SeqCst) };
            let updated_or = match &or.state {
                OperationState::Completed(result) => { 
                    let _ = self.help.try_remove_front(orb); 
                    return;
                }
                OperationState::PreCas => {
                    let mut contention = ContentionMeasure(0);
                    let cas_list = self.algorithm.generate(&or.input, &mut contention);
                    Box::new(OperationRecord {
                        owner: or.owner.clone(),
                        input: or.input.clone(),
                        state: OperationState::ExecuteCas(cas_list),
                    })
                }
                OperationState::ExecuteCas(cas_list) => {
                    let outcome = self.cas_execute(cas_list);
                    Box::new(OperationRecord {
                        owner: or.owner.clone(),
                        input: or.input.clone(),
                        state: OperationState::PostCas(cas_list.clone(), outcome),
                    })
                }
                OperationState::PostCas(cas_list, outcome) => {
                    let contention = ContentionMeasure(0);
                    if let Ok(result) = self.algorithm.wrap_up(*outcome, cas_list) {
                        Box::new(OperationRecord{
                                owner: or.owner.clone(),
                                input: or.input.clone(),
                                state: OperationState::Completed(result),
                        })
                    } else {
                        Box::new(OperationRecord{
                            owner: or.owner.clone(),
                            input: or.input.clone(),
                            state: OperationState::PreCas,
                        })
                    }
                }
            };
            let updated_or = Box::into_raw(updated_or);
            if orb.val.compare_exchange_weak(or as *mut OperationRecord<_>,
                                             updated_or as *mut OperationRecord<_>,
                                             Ordering::SeqCst, Ordering::SeqCst).is_err() {
                let _ = unsafe { Box::from_raw(updated_or) };
            }
        }
    }
    // WF wraps LF and provides run() that takes Input and ret Output, with cas in between.
    pub fn run(&self, op: LF::Input) -> LF::Output {
        let help = true;
        if help {
            self.help_first();
        }
        // fast path
        for retry in 0.. {
            let mut contention = ContentionMeasure(0);
            let cases = self.algorithm.generate(&op, &mut contention);
            let result = self.cas_execute(&cases);
            match self.algorithm.wrap_up(result, &cases) {
                Ok(outcome) => { return outcome; },
                Err(_) => {}
            }
            if retry > RETRY_THRESHOLD {
                break;
            }
        }
        // slow path, create a OpRecord and enqueu, then help_first
        let orb = OperationRecordBox {
            val: AtomicPtr::new(Box::into_raw(Box::new(OperationRecord {
                owner: thread::current().id(),
                input: op, // consume,
                state: OperationState::PreCas,
            }))),
        };
        self.help.enqueue(&orb);
        loop {
            let or = unsafe { &* orb.val.load(Ordering::SeqCst) };
            if let OperationState::Completed(t) = &or.state {
                break t.clone();
            } else {
                self.help_first();
            }
        }
    }
}


//
// client data struct impls NormalizedLockFree to generate struct specific CASes.
struct LockFreeLinkedList<T> {
    t: T,
}
impl<T> NormalizedLockFree for LockFreeLinkedList<T> {
}

// struct WaitFreeLinkedList<T> {
//     // all ops impled by WF simulator executor.
//     simulator: WaitFreeSimulator<LockFreeLinkedList<T>>,
// }
// impl<T> WaitFreeLinkedList<T> {
//     pub fn enqueue(&self, t: T) {
//         self.simulator.run(t);
//     }
// }

