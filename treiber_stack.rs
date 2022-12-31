// crossbeam's lock-free algorithm: epoch based reclaimation.
// lock-free means no mutex, use atomic CAS to swap data/pointer when publishing.
//
// Problem that one thread removes a *node* from a shared data structure
// while other threads still holds the ref to the removed node.
// 
// 1. When accessing a shared tree, a thread must "pin the current epoch"
// and get a Guard which unpin the epoch when destroyed.
// 2. Subsequent reads of a lock-free data structure, the pointers it extracts
// act like references with lifetime tied to the Guard. 
// 
// create Owned<T>, pin, get Shared<'a, T>, atomic swap Atomic<T>
// cas_and_ref(&guard) or cas_shard();
//
// the danger of atomic:
// 1. memory access re-order by compiler/cpu, or slower in weakly-ordered 
// archi like ARM/POWER.
// 2. program order issue, update atomic<bool> data_ready is independent of data update.
// 3. Sequences of atomic operations are then not atomic as a whole. ABA problem.
// you can not rely on the order of a set of stomics to protect independent memory locations.
// 
// 1. atomic<T>, T must be primitively simple. Complex type may incur mutex
// underhood,defeat the purpose.
// std::atomic<T*> is not simple as the mem pointed by T* need protection.
// 2. The property is set at most once inside the mutex lock.
// 3. switch to atomic from mutex may surface bugs that depends on mutex 
// memory order implicitly.

use std::sync::atomic::Ordering::{Acquire, Release, Relaxed};
use std::ptr;

use crossbeam::mem::epoch::{self, Atomic, Owned};

struct Node<T> {
    data: T,
    next: Atomic<Node<T>>,
}
// atomic cas swap next ptr when publishing/poping nodes.  
struct TreiberStack<T> {
    head: Atomic<Node<T>>,
}

impl<T> TreiberStack<T> {
  fn new() -> TreiberStack<T> {
    TreiberStack {
        head: Atomic::null()
    }
  }

  fn push(&self, t: T) {
    // create a locally owned node, before publishing 
    let mut local_new_node = Owned::new(Node {
      data: t,
      next: Atomic::null(),
    });

    // pin the current epoch to get a guard of snapshot
    let guard = epoch::pin();

    loop {
      // snapshot current head with Relaxed
      let head = self.head.load(Relaxed, &guard);

      // update local owned's `next` pointer to shared data from snapshot
      local_new_node.next.store_shared(head, Relaxed);

      // cas swap head's next to local owned node, Release, publishing. 
      match self.head.cas_and_ref(head, local_new_node, Release, &guard) {
        Ok(_) => return,
        Err(owned) => local_new_node = owned,
      }
    }
  }

  fn pop(&self) -> Option<T> {
    // pin the current epoch to get a guard of the snapshot
    let guard = epoch::pin();

    loop {
      // take a snapshot with Acquire
      match self.head.load(Acquire, &guard) {
        // the stack is non-empty
        Some(head) => {
          // head is shared ptr in guarded snapshot, read is *safely*!
          let next = head.next.load(Relaxed, &guard);
          // cas swap head with head's next.
          if self.head.cas_shared(Some(head), next, Release) {
            unsafe {
              // mark the node as unlinked in the guarded snapshot.
              guard.unlinked(head);
              // extract out the data from the now-unlinked node
              return Some(ptr::read(&(*head).data))
            }
          }
        }
        // we observed the stack empty
        None => return None
      }
    }
  }
}
