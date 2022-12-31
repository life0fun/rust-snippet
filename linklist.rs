#[warn(unused_imports)]
use std::mem;

pub mod first {
    #[derive(Debug)]
    enum BinEntry<T> {
        Empty,
        NodeEntry(Box<Node<T>>),  // wrap box node as a named new enum subtype. 
    }
    #[derive(Debug)]
    struct Node<T> {
        ele: T,
        next: BinEntry<T>, // next *Owns* the box, can not move it out of behind a node ref and leave value empty.
        //next: Option<NodeEntry(Box<Node<T>>),   // Explicit Option so next can be set to None.
    }
    // head owns the Boxed Node. &mut list, can not move list's head as the head is behind &mut.
    // leave the list value into inconsistent state after head moved out !!
    // https://stackoverflow.com/questions/68415514/how-can-i-swap-out-the-value-of-a-mutable-reference-temporarily-taking-ownershi
    // if head is owned by container/ptr [AtomicPtr, Option], then it can be pulled out from both &list.head and list.head 
    #[derive(Debug)]
    pub struct List<T> {
        // head owns BinEntry, with &mut list, can not move &mut list.head out of the List struct.
        // with consuming list, can not move list.head b/c Drop can not run as the head is moved out.
        // as the ownership of head BinEntry is changing,(list.head, list.head.node.next), it can not
        // be fixed owned by head, has to use a container/ptr.
        head: BinEntry<T>, 
    }
    // https://stackoverflow.com/questions/68444150/how-do-i-destructure-an-object-without-dropping-it?noredirect=1&lq=1
    // Moving data out of the value would leave it in an undefined state. That means that when 
    // Drop::drop is automatically run by the compiler, you'd be creating undefined behavior.
    // The contract you've created with the compiler by implementing Drop is that you have code that must run 
    // when an X is destroyed, and that X must be complete to do so. Destructuring is antithetical to that contract.
    impl<T> Drop for List<T> {
        fn drop(&mut self) {
            let mut old_head = std::mem::replace(&mut self.head, BinEntry::Empty);
            while let BinEntry::NodeEntry(mut n) = old_head {
                old_head = std::mem::replace(&mut n.next, BinEntry::Empty);
            }
        }
    }
    impl<T> List<T> {
        pub fn new() -> Self {
            List {
                head: BinEntry::Empty,
            }
        }
        // Can not pull apart fields from a struct as it cause the struct to inconsistent state !!
        // must use a container, Option, to store fields that can be pulled out of a struct.
        // pub fn push_consume_self(self, ele: T) -> Self {
        //     let node = Node {
        //         ele,
        //         next: self.head  // pull head apart from list, Drop of list can not run !
        //     };
        //     List {
        //         head: BinEntry::NodeEntry(Box::new(node)),
        //     }
        // }
        pub fn push_head(&mut self, ele: T) {
            let old_head = std::mem::replace(&mut self.head, BinEntry::Empty);
            let node = Node {
                ele,
                next: old_head,
            };
            self.head = BinEntry::NodeEntry(Box::new(node));
        }
        pub fn push_tail(&mut self, _: T) {}
        pub fn pop_head(&mut self) -> Option<T> {
            let old_head = std::mem::replace(&mut self.head, BinEntry::Empty);
            match old_head {
                BinEntry::Empty => None,
                BinEntry::NodeEntry(boxed_node) => {
                    std::mem::replace(&mut self.head, boxed_node.next);
                    // self.head = boxed_node.next;
                    Some(boxed_node.ele)
                }
            }
        }
        pub fn pop_tail(&mut self) -> Option<T> {
            None
        }
        pub fn peek_head(&self) -> Option<&T> {
            match &self.head {
                BinEntry::Empty => None,
                BinEntry::NodeEntry(node) => Some(&node.ele),
            }
        }
        pub fn peek_tail(&self) -> Option<&T> {
            None
        }
    }
}

pub mod second {
    // adding Option<T> container for head/next pointer for it can be pulled out/reset and left with None.
    // Moving objects out of structs is possible as long as they are stored in an intermediate container 
    // that supports moving. Option is such a container, and its take() method is designed for exactly that purpose.
    // type Entry<T> = Option<Box<Node<T>>>; 
    #[derive(Debug)]
    pub struct Node<T> {
        ele: T,
        next: Option<Box<Node<T>>>, 
    }
    #[derive(Debug)]
    pub struct List<T> {
        head: Option<Box<Node<T>>>,
    }
    pub struct IntoIter<T>(List<T>);
    pub struct ListIter<'a, T> {
        next: Option<&'a Node<T>>,
    }
    pub struct ListIterMut<'a, T> {
        next: Option<&'a mut Node<T>>,
    }
    impl<T> List<T> {
        pub fn new() -> Self {
            Self { head: None }
        }
        pub fn push_head(&mut self, ele: T) {
            let node = Node {
                ele,
                next: self.head.take(),
            };
            let new_hd = Box::new(node);
            self.head = Some(new_hd);
        }
        pub fn pop_head(&mut self) -> Option<T> {
            self.head.take().map(|node| {
                self.head = node.next;
                node.ele
            })
        }
        pub fn peek_head(&self) -> Option<&T> {
            // cast &Option<Box<T>> to Option<&T>, map consumes *that* &T
            self.head.as_ref().map(|node| &node.ele)
        }
        pub fn peek_head_mut(&mut self) -> Option<&mut T> {
            self.head.as_mut().map(|node| &mut node.ele)
        }
        pub fn into_iter(self) -> IntoIter<T> {
            IntoIter(self) // self consumed and moved
        }
        pub fn iter<'a>(&'a mut self) -> ListIter<'a, T> {
            ListIter {
                // Option<Box<T>>, as_ref() => {&**inner}
                next: self.head.as_deref(),
            }
        }
        pub fn iter_mut<'a>(&'a mut self) -> ListIterMut<'a, T> {
            ListIterMut {
                next: self.head.as_deref_mut(),
            }
        }
    }
    impl<T> Iterator for IntoIter<T> {
        type Item = T;
        fn next(&mut self) -> Option<Self::Item> {
            self.0.pop_head()
        }
    }
    impl<'a, T> Iterator for ListIter<'a, T> {
        type Item = &'a T;
        fn next(&mut self) -> Option<Self::Item> {
            self.next.take().map(|node| {
                // self.next = node.next.as_ref().map::<&Node<T>, _>(|nn| {&**nn});
                self.next = node.next.as_deref();
                &node.ele
            })
        }
    }
    impl<'a, T> Iterator for ListIterMut<'a, T> {
        type Item = &'a mut T;
        fn next(&mut self) -> Option<Self::Item> {
            self.next.take().map(|node| {
                self.next = node.next.as_deref_mut();
                &mut node.ele
            })
        }
    }
    impl<T> Drop for List<T> {
        fn drop(&mut self) {
            while let Some(mut box_node) = self.head.take() {
                self.head = box_node.next.take();
            }
        }
    }
}

// A node is shared by both next and prev.
pub mod RcLinkList {
    use core::fmt::Debug;
    use std::cell::{Ref, RefCell, RefMut};
    use std::ops::Deref;
    use std::rc::Rc;

    type Entry<T> = Option<Rc<RefCell<Node<T>>>>;
    enum BinEntry<T> {
        Empty,
        NodeEntry(Rc<RefCell<Node<T>>>),
    }
    #[derive(Debug)]
    pub struct Node<T> {
        pub ele: T,
        prev: Option<Rc<RefCell<Node<T>>>>, // Node is shared by both prev and next pointers 
        next: Option<Rc<RefCell<Node<T>>>>,
    }
    #[derive(Debug)]
    pub struct List<T> {
        head: Option<Rc<RefCell<Node<T>>>>, // own the Rc, not a ref
        tail: Option<Rc<RefCell<Node<T>>>>,
    }
    // struct IntoIter is a tuple struct wraps List<T>
    pub struct IntoIter<T>(List<T>);
    pub struct ListIter<'a, T> {
        next: Option<Ref<'a, Node<T>>>,
    }
    pub struct ListIterMut<'a, T> {
        next: Option<RefMut<'a, Node<T>>>,
    }
    impl<T> List<T> {
        fn new_node(ele: T) -> Rc<RefCell<Node<T>>> {
            Rc::new(RefCell::new(Node {
                ele,
                prev: None,
                next: None,
            }))
        }
        pub fn new() -> Self {
            Self {
                head: None,
                tail: None,
            }
        }
        pub fn push_head(&mut self, ele: T) {
            let old_head = self.head.take();
            let new_head = List::new_node(ele);
            new_head.borrow_mut().next = old_head.clone();
            match old_head {
                Some(hd) => {
                    hd.borrow_mut().prev = Some(new_head.clone());
                }
                None => {
                    self.tail = Some(new_head.clone());
                }
            }
            self.head = Some(new_head);
        }
        pub fn push_tail(&mut self, ele: T) {
            let old_tail = self.tail.take();
            let new_tail = List::new_node(ele);
            new_tail.borrow_mut().prev = old_tail.clone();
            match old_tail {
                Some(tail) => {
                    tail.borrow_mut().next = Some(new_tail.clone());
                }
                None => {
                    self.head = Some(new_tail.clone());
                }
            }
            self.tail = Some(new_tail);
        }
        pub fn pop_head(&mut self) -> Option<T> {
            self.head.take().map(|head| {
                // self.head = refcell_node.borrow().next.as_ref().map(|rc| { rc.clone() });
                self.head = head.borrow().next.clone();
                self.head.as_ref().map(|n| {
                    n.borrow_mut().prev = None;
                });
                if self.head.is_none() {
                    self.tail = None;
                }
                // Safety: sure Rc ref is 0 now after pop.
                Rc::try_unwrap(head).ok().unwrap().into_inner().ele
            })
        }
        pub fn pop_tail(&mut self) -> Option<T> {
            self.tail.take().map(|tail| {
                self.tail = tail.borrow().prev.clone();
                self.tail.as_ref().map(|n| {
                    n.borrow_mut().next = None;
                });
                if self.tail.is_none() {
                    self.head = None;
                }
                // Safety: sure Rc ref is 0 now after pop.
                Rc::try_unwrap(tail).ok().unwrap().into_inner().ele
            })
        }
        pub fn peek_head(&self) -> Option<Ref<'_, T>> {
            self.head
                .as_ref()
                .map(|node| Ref::map(node.borrow(), |n| &n.ele)) // Ref<Cell> => Ref<T>
        }
        pub fn peek_tail(&self) -> Option<Ref<'_, T>> {
            match self.tail.as_ref() {
                Some(tail) => Some(Ref::map(tail.borrow(), |n| &n.ele)),
                None => None,
            }
        }
        pub fn peek_mut(&mut self) -> Option<RefMut<'_, T>> {
            self.head
                .as_mut()
                .map(|node| RefMut::map(node.borrow_mut(), |rn| &mut rn.ele))
        }
        pub fn into_iter(self) -> IntoIter<T> {
            IntoIter(self) // self consumed and moved
        }
        pub fn iter<'a>(&'a mut self) -> ListIter<'a, T> {
            ListIter {
                next: self.head.as_ref().map(|node| node.borrow()),
            }
        }
    }
    impl<T> Iterator for IntoIter<T> {
        type Item = T;
        fn next(&mut self) -> Option<Self::Item> {
            self.0.pop_head()
        }
    }
    impl<T> DoubleEndedIterator for IntoIter<T> {
        fn next_back(&mut self) -> Option<Self::Item> {
            self.0.pop_tail()
        }
    }
    impl<'a, T> Iterator for ListIter<'a, T> {
        type Item = Ref<'a, T>;
        fn next(&mut self) -> Option<Self::Item> {
            let next = self.next.take(); // next is ref to Node
            next.map(|ref_node| {
                // self.next = ref_node.next.as_ref().map(|head| { head.borrow() });
                Ref::map(ref_node, |node| &node.ele)
            })
            // next.map(|ref_node| {Ref::map(ref_node, |n| {&n.ele})})
            // next.map(|node_ref| {
            //     let (next, ele) = Ref::map_split(node_ref, |node| {
            //         (&node.next, &node.ele)
            //     });
            //     self.next = next.as_ref().map(|head| head.borrow());
            //     ele
            // })
        }
    }
    impl<T> Drop for List<T> {
        fn drop(&mut self) {
            while self.pop_head().is_some() {}
        }
    }
}
// no borrow check of List's tail pointer.
pub mod RawPtrLinkList {
    use core::fmt::Debug;
    use std::cell::{Ref, RefCell, RefMut};
    use std::ops::Deref;
    use std::rc::Rc;

    type Entry<T> = Option<Box<Node<T>>>;
    
    #[derive(Debug)]
    pub struct Node<T> {
        pub ele: T,
        next: Option<Box<Node<T>>>,
    }
    #[derive(Debug)]
    pub struct List<T> {
        head: Option<Box<Node<T>>>,
        tail: *mut Node<T>,  
    }
    // struct IntoIter is a tuple struct wraps List<T>
    pub struct IntoIter<T>(List<T>);
    // pub struct ListIter<'a, T>(Option<Ref<'a, Node<T>>)
    // pub struct ListIterMut<'a, T>(Option<RefMut<'a, Node<T>>>)

    impl<T> List<T> {
        fn new_node(ele: T) -> Box<Node<T>> {
            Box::new(Node { ele, next: None })
        }
        pub fn new() -> Self {
            Self {
                head: None,
                tail: std::ptr::null_mut(),
            }
        }
        pub fn push_head(&mut self, ele: T) {
            let mut new_head = List::new_node(ele);
            self.head.take().map(|node| {new_head.next = Some(node);});
            if self.tail.is_null() {
                self.tail = &mut *new_head; // explicit cast to *mut _
            }
            self.head = Some(new_head);
        }
        pub fn push_tail(&mut self, ele: T) {
            let mut new_tail = List::new_node(ele);
            let raw_tail: *mut _ = &mut *new_tail; // explicit cast to *mut _
            if !self.tail.is_null() {
                unsafe {
                    (*self.tail).next = Some(new_tail);
                }
            } else {
                self.head = Some(new_tail);
            }
            self.tail = raw_tail;
        }
        pub fn pop_head(&mut self) -> Option<T> {
            self.head.take().map(|head| {
                self.head = head.next;
                if self.head.is_none() {
                    self.tail = std::ptr::null_mut();
                }
                head.ele
            })
        }
        pub fn peek_head(&self) -> Option<&T> {
            self.head.as_ref().map(|head| &head.ele)
        }
        pub fn peek_tail(&self) -> Option<&T> {
            if !self.tail.is_null() {
                return Some(unsafe { (&(*self.tail).ele) });
            }
            None
        }
    }
    // impl<T> Iterator for IntoIter<T> {
    //     type Item = T;
    //     fn next(&mut self) -> Option<Self::Item> {
    //         self.0.pop_head()
    //     }
    // }
    // impl<T> Drop for List<T> {
    //     fn drop(&mut self) {
    //         // while self.pop_head().is_some() {}
    //     }
    // }
}

pub mod RawPtrList2 {
    use core::fmt::Debug;
    use std::cell::{Ref, RefCell, RefMut};
    use std::ops::Deref;
    use std::ptr;
    use std::rc::Rc;

    type Entry<T> = *mut Node<T>;
    enum BinEntry<T> {
        Empty,
        NodeEntry(*mut Node<T>),
    }
    #[derive(Debug)]
    pub struct Node<T> {
        pub ele: T,
        next: *mut Node<T>,
    }
    #[derive(Debug)]
    pub struct List<T> {
        head: *mut Node<T>,
        tail: *mut Node<T>,
    }
    // struct IntoIter is a tuple struct wraps List<T>
    pub struct IntoIter<T>(List<T>);
    pub struct ListIter<'a, T>(Option<&'a Node<T>>);
    pub struct ListIterMut<'a, T>(Option<&'a mut Node<T>>);

    impl<T> List<T> {
        fn new_node(ele: T) -> Box<Node<T>> {
            Box::new(Node {
                ele,
                next: std::ptr::null_mut(),
            })
        }
        pub fn new() -> Self {
            Self {
                head: std::ptr::null_mut(),
                tail: std::ptr::null_mut(),
            }
        }
        pub fn push_tail(&mut self, ele: T) {
            let mut new_tail = List::new_node(ele);
            let raw_tail = Box::into_raw(new_tail);
            if !self.tail.is_null() {
                unsafe {
                    (*self.tail).next = raw_tail;
                }
            } else {
                self.head = raw_tail;
            }
            self.tail = raw_tail;
        }
        pub fn pop_head(&mut self) -> Option<T> {
            if !self.head.is_null() {
                unsafe {
                    let old_head = Box::from_raw(self.head);
                    self.head = old_head.next;

                    if self.head.is_null() {
                        self.tail = std::ptr::null_mut();
                    }
                    return Some(old_head.ele);
                }
            }
            None
        }
        pub fn peek_head(&self) -> Option<&T> {
            if !self.head.is_null() {
                let node = unsafe { &*self.head };
                return Some(&node.ele);
            }
            None
        }
        pub fn peek_tail(&self) -> Option<&T> {
            if !self.tail.is_null() {
                let node = unsafe { &*self.tail };
                return Some(&node.ele);
            }
            None
        }
        pub fn into_iter(self) -> IntoIter<T> {
            IntoIter(self)
        }
        pub fn iter(&self) -> ListIter<'_, T> {
            unsafe { ListIter(self.head.as_ref()) }
        }
        pub fn iter_mut(&mut self) -> ListIterMut<'_, T> {
            unsafe { ListIterMut(self.head.as_mut()) }
        }
    }
    impl<T> Iterator for IntoIter<T> {
        type Item = T;
        fn next(&mut self) -> Option<Self::Item> {
            self.0.pop_head()
        }
    }
    impl<'a, T> Iterator for ListIter<'a, T> {
        type Item = Option<&'a T>;
        fn next(&mut self) -> Option<Self::Item> {
            self.0.take().map(|node| {
                unsafe {
                    self.0 = node.next.as_ref();
                }
                Some(&node.ele)
            })
        }
    }
    impl<T> Drop for List<T> {
        fn drop(&mut self) {
            while let Some(_) = self.pop_head() {}
        }
    }
}

pub mod RawPtrList3 {
    use core::fmt::Debug;
    use core::marker::PhantomData;
    use std::cell::{Ref, RefCell, RefMut};
    use std::ops::Deref;
    use std::ptr;
    use std::ptr::NonNull;
    use std::rc::Rc;
    // NonNull ptr to Node<T> impls Copy, Option<NonNull> is copyable.
    type Entry<T> = Option<NonNull<Node<T>>>;
    enum BinEntry<T> {
        Empty,
        NodeEntry(Option<NonNull<Node<T>>>),
    }
    #[derive(Debug)]
    pub struct Node<T> {
        pub ele: T,
        prev: Option<NonNull<Node<T>>>,
        next: Option<NonNull<Node<T>>>,
    }
    #[derive(Debug)]
    pub struct List<T> {
        head: Option<NonNull<Node<T>>>, // nullable ptr
        tail: Option<NonNull<Node<T>>>,
        len: usize,
        _ghost: PhantomData<T>, // protect NonNull ptr that stores T.
    }
    pub struct IntoIter<T>(List<T>);
    pub struct ListIter<'a, T>(Option<&'a Node<T>>);
    pub struct ListIterMut<'a, T>(Option<&'a mut Node<T>>);

    impl<T> List<T> {
        pub fn new() -> Self {
            Self {
                head: None,
                tail: None,
                len: 0,
                _ghost: PhantomData,
            }
        }
        fn new_nonnull(ele: T) -> NonNull<Node<T>> {
            unsafe {
                NonNull::new_unchecked(Box::into_raw(Box::new(Node {
                    ele,
                    prev: None,
                    next: None,
                })))
            }
        }
        pub fn push_head(&mut self, ele: T) {
            let mut new_head = List::new_nonnull(ele);
            if let Some(mut old) = self.head {
                // Opt<NonNull<> copyable, destructure won't move
                unsafe {
                    // NonNull ptr to Node<T> derefed to &mut T via as_ptr() or as_mut()
                    old.as_mut().prev = Some(new_head);
                    (*new_head.as_ptr()).next = Some(old);
                }
            } else {
                self.tail = Some(new_head);
            }
            self.len += 1;
            self.head = Some(new_head); // NonNull impl Copy Trait
        }
        pub fn push_tail(&mut self, ele: T) {
            let mut new_tail = List::new_nonnull(ele);
            if let Some(old_tail) = self.tail {
                unsafe {
                    (*old_tail.as_ptr()).next = Some(new_tail);
                    new_tail.as_mut().prev = Some(old_tail);
                }
            } else {
                self.head = Some(new_tail);
            }
            self.len += 1;
            self.tail = Some(new_tail);
        }
        pub fn pop_head(&mut self) -> Option<T> {
            self.head.take().map(|node_nnptr| {
                unsafe {
                    // the raw pointer is owned by the resulting Box
                    let boxed_node = Box::from_raw(node_nnptr.as_ptr());
                    self.head = boxed_node.next;
                    if let Some(hd) = self.head {
                        (*hd.as_ptr()).prev = None; // not circle back to self.tail
                    } else {
                        self.tail = None;
                    }
                    self.len -= 1;
                    boxed_node.ele // mov ele out of box and ret. box implicitly dropped.
                }
            })
        }
        pub fn pop_tail(&mut self) -> Option<T> {
            self.tail.take().map(|tail| {
                unsafe {
                    let boxed_node = Box::from_raw(tail.as_ptr());
                    self.tail = boxed_node.prev;
                    if let Some(tail) = self.tail {
                        (*tail.as_ptr()).next = None;
                    } else {
                        self.head = None;
                    }
                    boxed_node.ele // mov ele out of box, box implicitly dropped.
                }
            })
        }
        pub fn peek_head(&self) -> Option<&T> {
            unsafe { Some(&(*self.head?.as_ptr()).ele) }
        }
        pub fn peek_tail(&self) -> Option<&T> {
            unsafe { Some(&(*self.tail.as_ref()?.as_ptr()).ele) }
        }
        pub fn into_iter(self) -> IntoIter<T> {
            IntoIter(self)
        }
        pub fn iter(&self) -> ListIter<'_, T> {
            unsafe {
                let it = self.head.map(|nonnull| {
                    &(*nonnull.as_ptr()) // deref box ptr to node
                });
                ListIter(it)
            }
        }
        pub fn iter_mut(&mut self) -> ListIterMut<'_, T> {
            unsafe {
                let it = self.head.map(|mut nonnull| {
                    nonnull.as_mut()
                    // &mut (*nonnull.as_ptr())
                });
                ListIterMut(it)
            }
        }
        // List is WLocked by the reted cursor.
        pub fn cursor_mut(&mut self) -> CursorMut<T> {
            let mut idx = None;
            if self.head.is_some() {
                idx = Some(0);
            }
            let cur = self.head;
            CursorMut {
                /*&mut List<T>*/ list: self,
                cur: cur,
                index: idx,
            }
        }
    }
    pub struct CursorMut<'a, T> {
        list: &'a mut List<T>, // WLocked the list during the cursor moving.
        cur: Entry<T>,         // copyable
        index: Option<usize>,
    }
    impl<T> Iterator for IntoIter<T> {
        type Item = T;
        fn next(&mut self) -> Option<Self::Item> {
            self.0.pop_head()
        }
    }
    impl<'a, T> Iterator for ListIter<'a, T> {
        type Item = &'a T;
        fn next(&mut self) -> Option<Self::Item> {
            self.0.take().map(|node| {
                // Opt<&Node>
                unsafe {
                    node.next.map(|next_nnptr| {
                        // destruct a NonNull<Node<T>> ptr.
                        self.0 = Some(&(*next_nnptr.as_ptr()));
                    });
                    &node.ele
                }
            })
        }
    }
    impl<'a, T> Iterator for ListIterMut<'a, T> {
        type Item = &'a mut T;
        fn next(&mut self) -> Option<Self::Item> {
            self.0.take().map(|node| unsafe {
                node.next.as_mut().map(|next_nnptr| {
                    self.0 = Some(&mut (*next_nnptr.as_ptr()));
                });
                &mut node.ele
            })
        }
    }
    impl<T> Extend<T> for List<T> {
        fn extend<I: IntoIterator<Item = T>>(&mut self, iter: I) {
            for ele in iter {
                self.push_tail(ele);
            }
        }
    }
    impl<'a, T> CursorMut<'a, T> {
        pub fn index(&self) -> Option<usize> {
            self.index
        }
        pub fn current(&mut self) -> Option<&mut T> {
            self.cur.map(|mut node| unsafe { &mut node.as_mut().ele })
        }
        pub fn at(&mut self, i: usize) -> Entry<T> {
            let mut offset = 0;
            let mut cur = self.list.head;
            while offset < i {
                if let Some(n) = cur {
                    cur = unsafe { n.as_ref().next };
                    offset += 1;
                } else {
                    return None;
                }
            }
            cur
        }
        pub fn move_next(&mut self) {
            self.cur.take().map(|node| unsafe {
                self.cur = node.as_ref().next;
            });
            if self.cur.is_none() {
                self.index = None;
            } else {
                self.index = self.index.map(|idx| idx + 1);
            }
        }
        pub fn move_prev(&mut self) {
            if let Some(idx) = self.index {
                if idx == 0 {
                    self.cur = None;
                    self.index = None;
                    return;
                }
                self.cur = self.at(idx - 1);
                self.index = Some(idx - 1);
            }
        }
        pub fn peek_next(&mut self) -> Option<&mut T> {
            // if let Some(idx) = self.index {
            //     let mut next = self.at(idx + 1);
            //     next.as_mut().map(|mut node| {
            //       return unsafe { &mut node.as_mut().ele };
            //     });
            // }
            // None
            unsafe {
                self.cur
                    .and_then(|mut node_nnptr| node_nnptr.as_mut().next)
                    .map(|mut node| &mut node.as_mut().ele)
            }
        }
        pub fn peek_prev(&mut self) -> Option<&mut T> {
            unsafe {
                self.cur
                    .and_then(|mut cur_nn| cur_nn.as_mut().prev)
                    .map(|mut prev_node| &mut prev_node.as_mut().ele)
            }
        }
        pub fn split_before(&mut self) -> List<T> {
            if self.cur.is_none() {
                return List::new();
            }
            let head = self.cur;
            unsafe {
                let next_head = head.and_then(|mut node_nnptr| node_nnptr.as_mut().next);
                if next_head.is_some() {
                    let idx = self.index.unwrap();
                    self.list.tail = head;
                    head.map(|mut node_nnptr| {
                        node_nnptr.as_mut().next = None;
                    });
                    next_head.map(|mut node_nnptr| {
                        node_nnptr.as_mut().prev = None;
                    });
                    List {
                        head: next_head,
                        tail: self.list.tail,
                        len: self.list.len - idx,
                        _ghost: PhantomData,
                    }
                } else {
                    List::new()
                }
            }
        }
        pub fn splice_before(&mut self, mut input: List<T>) {}
    }
    impl<T> Drop for List<T> {
        fn drop(&mut self) {
            while let Some(_) = self.pop_head() {}
        }
    }
}

use crate::RawPtrList3::List;
fn main() {
    {
        let mut list = List::new();
        list.push_head(String::from("a"));
        list.push_tail(String::from("z"));
        list.push_head(String::from("b"));
        list.push_tail(String::from("y"));
        list.push_head(String::from("c"));
        println!("pop_head = {:?}", list.pop_head());
        println!(
            "head = {:?}, tail = {:?}",
            list.peek_head(),
            list.peek_tail()
        );

        list.push_tail(String::from("x"));
        println!(
            "head = {:?}, tail = {:?}",
            list.peek_head(),
            list.peek_tail()
        );

        println!("pop_tail = {:?}", list.pop_tail());
        println!(
            "head = {:?}, tail = {:?}",
            list.peek_head(),
            list.peek_tail()
        );

        println!("pop_head = {:?}", list.pop_head());
        println!(
            "head = {:?}, tail = {:?}",
            list.peek_head(),
            list.peek_tail()
        );

        println!("pop_head = {:?}", list.pop_head());
        println!(
            "head = {:?}, tail = {:?}",
            list.peek_head(),
            list.peek_tail()
        );
        println!("pop_head = {:?}", list.pop_head());
        println!(
            "head = {:?}, tail = {:?}",
            list.peek_head(),
            list.peek_tail()
        );
        println!("pop_tail = {:?}", list.pop_tail());
        println!(
            "head = {:?}, tail = {:?}",
            list.peek_head(),
            list.peek_tail()
        );
        assert!(list.pop_tail().is_none());
        assert!(list.pop_head().is_none());
    }
    { // cursor_mut()
        let mut list = List::new();
        list.push_head(String::from("a"));
        list.push_head(String::from("z"));
        list.push_head(String::from("b"));
        list.push_head(String::from("y"));
        list.push_head(String::from("c"));
        list.push_head(String::from("x"));

        let mut cursor = list.cursor_mut(); // cursor WLocked the list.
        while !cursor.index().is_none() {
            println!("cursor = {:?}", cursor.current());
            cursor.move_next();
        }
        println!(
            "head = {:?}, tail = {:?}",
            list.peek_head(),
            list.peek_tail()
        );

        let mut cursor = list.cursor_mut(); // cursor WLocked the list.
        println!("cursor = {:?}", cursor.current());
        cursor.move_next();
        cursor.move_next();
        println!("cursor = {:?}", cursor.current());
        cursor.move_prev();
        println!("cursor = {:?}", cursor.current());
        cursor.move_prev();
        println!("cursor = {:?}", cursor.current());
        cursor.move_prev();
        println!("cursor = {:?}", cursor.current());
        cursor.move_prev();
        println!("cursor = {:?}", cursor.current());

        println!(
            "head = {:?}, tail = {:?}",
            list.peek_head(),
            list.peek_tail()
        );

        let mut list = List::new();
        list.push_head(String::from("a"));
        list.push_head(String::from("z"));
        list.push_head(String::from("b"));
        list.push_head(String::from("y"));
        list.push_head(String::from("c"));
        list.push_head(String::from("x"));
        let mut cursor = list.cursor_mut(); // cursor WLocked the list.
        println!("current = {:?}", cursor.current());

        cursor.move_next();
        println!("current = {:?}", cursor.current());
        println!("peek_next = {:?}", cursor.peek_next());

        cursor.move_next();
        println!("current = {:?}", cursor.current());
        println!("peek_next = {:?}", cursor.peek_next());
    }

    {
        let mut list = List::new();
        list.push_head(String::from("a"));
        list.push_tail(String::from("z"));
        list.push_head(String::from("b"));
        list.push_tail(String::from("y"));
        list.push_head(String::from("c"));
        list.push_tail(String::from("x"));

        let mut intoiter = list.into_iter();
        for e in intoiter {
            println!("{:?}", e);
        }
    }
    {
        let mut list = List::new();
        list.push_head(String::from("a"));
        list.push_tail(String::from("z"));
        list.push_head(String::from("b"));
        list.push_tail(String::from("y"));
        list.push_head(String::from("c"));
        list.push_tail(String::from("x"));

        for e in list.iter() {
            println!("list.iter.ele = {:?}", e);
        }
    }
}
