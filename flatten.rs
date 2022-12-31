pub struct Flatten<O>
where
    O: Iterator,
    O::Item: IntoIterator,
{
    outer: O,
    front_inner: Option<<O::Item as IntoIterator>::IntoIter>,
    back_inner: Option<<O::Item as IntoIterator>::IntoIter>,
}

impl<O> Flatten<O>
where
    O: Iterator,
    O::Item: IntoIterator,
{
    pub fn new(iter: O) -> Self {
        Self {
            outer: iter,
            front_inner: None,
            back_inner: None,
        }
    }
}

impl<O> Iterator for Flatten<O>
where
    O: Iterator,
    O::Item: IntoIterator,
{
    type Item = <O::Item as IntoIterator>::Item;
    // iter owns items, next move the item out of iterator when return.
    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if let Some(ref mut front_inner) = self.front_inner {
                if let Some(item) = front_inner.next() {
                    return Some(item);
                }
                self.front_inner = None;
                continue;
            }
            if let Some(front_inner_iter) = self.outer.next() {
                // ASSIGN_OR_RETURN(self.inner, self.outer.next());
                self.front_inner = Some(front_inner_iter.into_iter());
            } else {
                return self.back_inner.as_mut()?.next();
            }
        }
    }
}

impl<O> DoubleEndedIterator for Flatten<O>
where
    O: DoubleEndedIterator,
    O::Item: IntoIterator,
    <O::Item as IntoIterator>::IntoIter: DoubleEndedIterator,
{
    fn next_back(&mut self) -> Option<Self::Item> {
        loop {
            if let Some(ref mut back_inner) = self.back_inner {
                if let Some(item) = back_inner.next_back() {
                    return Some(item);
                }
                self.back_inner = None;
                continue;
            }
            if let Some(outer_back) = self.outer.next_back() {
                self.back_inner = Some(outer_back.into_iter());
            } else {
                return self.front_inner.as_mut()?.next_back();
            }
        }
    }
}

// take a IntoIterator nested object, consume it and convert into an iterator object flatten nested
pub fn flatten<I>(into_iter: I) -> Flatten<I::IntoIter>
where
    I: IntoIterator,
    I::Item: IntoIterator,
{
    Flatten::new(into_iter.into_iter())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn two() {
        assert_eq!(flatten(vec![vec!["a"], vec!["b"]]).count(), 2);
    }
    #[test]
    fn two_wide() {
        assert_eq!(flatten(vec![vec!["a", "b"], vec!["c"]]).count(), 3);
    }
    #[test]
    fn both_end() {
        let mut iter = flatten(vec![vec!["a1", "a2", "a3", "a4"], vec!["b1", "b2", "b3"]]);
        assert_eq!(iter.next(), Some("a1"));
        assert_eq!(iter.next_back(), Some("b3"));
        assert_eq!(iter.next(), Some("a2"));
        assert_eq!(iter.next_back(), Some("b2"));
        assert_eq!(iter.next_back(), Some("b1"));
        assert_eq!(iter.next_back(), Some("a4"));
        assert_eq!(iter.next_back(), Some("a3"));
        assert_eq!(iter.next(), None);
        assert_eq!(iter.next_back(), None);
    }
}
