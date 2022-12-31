use std::collections::VecDeque;
use std::ops::DerefMut;
use std::sync::Arc;
use std::sync::Condvar;
use std::sync::Mutex;

struct Inner<T> {
    queue: VecDeque<T>,
    senders: usize,
}
struct Shared<T> {
    inner: Mutex<Inner<T>>,
    available: Condvar,
}

struct Sender<T> {
    shared: Arc<Shared<T>>,
}
impl<T> Sender<T> {
    pub fn new(shared: Arc<Shared<T>>) -> Self {
        Sender { shared }
    }
    pub fn send(&self, data: T) {
        let mut inner = self.shared.inner.lock().unwrap();
        inner.queue.push_back(data);
        drop(inner); // explicit drop inner guard
        self.shared.available.notify_one();
    }
}
impl<T> Clone for Sender<T> {
    fn clone(&self) -> Self {
        Sender {
            shared: Arc::clone(&self.shared),
        }
    }
}
impl<T> Drop for Sender<T> {
    fn drop(&mut self) {
        let mut inner = self.shared.inner.lock().unwrap();
        inner.senders -= 1;
        self.shared.available.notify_one();
    }
}

struct Recver<T> {
    shared: Arc<Shared<T>>,
    local_cache: VecDeque<T>,
}
impl<T> Recver<T> {
    pub fn new(shared: Arc<Shared<T>>) -> Self {
        Recver {
            shared,
            local_cache: VecDeque::default(),
        }
    }
    pub fn recv(&mut self) -> Option<T> {
        let shared = &self.shared;
        let avail = &shared.available;
        if let Some(t) = self.local_cache.pop_front() {
            return Some(t);
        }
        let mut inner = shared.inner.lock().unwrap(); // panic in place
        loop {
            match inner.queue.pop_front() {
                Some(t) => {
                    if !inner.queue.is_empty() {
                        std::mem::swap(&mut self.local_cache, &mut inner.queue);
                    }
                    return Some(t);
                }
                None => {
                    if dbg!(inner.senders) > 0 {
                        inner = avail.wait(inner).unwrap();
                    } else {
                        return None;
                    }
                }
            }
        }
    }
}
impl<T> Iterator for Recver<T> {
    type Item = T;
    fn next(&mut self) -> Option<Self::Item> {
        self.recv()
    }
}

fn channel<T>() -> (Sender<T>, Recver<T>) {
    let inner = Inner {
        queue: VecDeque::default(),
        senders: 1,
    };
    let shared = Shared {
        inner: Mutex::new(inner),
        available: Condvar::new(),
    };
    let shared = Arc::new(shared);
    let s = Sender::new(shared.clone());
    let r = Recver::new(shared.clone());

    (s, r)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ping_pong() {
        let (tx, mut rx) = channel();
        tx.send(1);
        assert_eq!(rx.recv(), Some(1));
    }
    #[test]
    fn closed() {
        let (tx, mut rx) = channel::<()>();
        // let _ = tx;
        drop(tx);
        assert_eq!(rx.recv(), None);
    }
}
