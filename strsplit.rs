#[derive(Debug)]
struct StrSplit<'a, D> {
    remainder: Option<&'a str>,
    delimiter: D,
}

impl<'a, D> StrSplit<'a, D>
where
    D: Delimiter,
{
    pub fn new(haystack: &'a str, delimiter: D) -> Self {
        Self {
            remainder: Some(haystack),
            delimiter,
        }
    }
}

pub trait Delimiter {
    fn find_next(&self, text: &str) -> Option<(usize, usize)>;
}

impl<'a, D> Iterator for StrSplit<'a, D>
where
    D: Delimiter,
{
    type Item = &'a str;
    // fn next(&mut self) -> Option<Self::Item> {
    //     if let Some(ref mut remainder) = self.remainder {
    //         if let Some(next) = remainder.find(self.delimiter) {
    //             let until = &remainder[..next];
    //             *remainder = &remainder[(next + self.delimiter.len())..];
    //             Some(until)
    //         } else {
    //             self.remainder.take()
    //         }
    //     } else {
    //         None
    //     }
    // }
    // fn next(&mut self) -> Option<Self::Item> {
    //     let ret = match self.remainder {
    //         None => { None },
    //         // Some("") => None, //self.remainder.take(),
    //         Some(remainder) => {
    //             if let Some(next_delim) = remainder.find(self.delimiter) {
    //                 let until_delimiter = &remainder[..next_delim];
    //                 self.remainder = Some(&remainder[(next_delim + self.delimiter.len())..]);
    //                 Some(until_delimiter)
    //             } else {
    //                 self.remainder.take()
    //             }
    //         },
    //     };
    //     ret
    // }
    fn next(&mut self) -> Option<Self::Item> {
        if let Some(ref mut remainder) = self.remainder {
            if let Some((start, end)) = self.delimiter.find_next(remainder) {
                let cur = &remainder[..start];
                *remainder = &remainder[end..];
                Some(cur)
            } else {
                self.remainder.take()
            }
        } else {
            None
        }
    }
}

impl Delimiter for &str {
    fn find_next(&self, text: &str) -> Option<(usize, usize)> {
        text.find(self).map(|pos| (pos, pos + self.len()))
    }
}
impl Delimiter for char {
    fn find_next(&self, text: &str) -> Option<(usize, usize)> {
        text.char_indices()
            .find(|(_, c)| c == self)
            .map(|(start, _)| (start, start + 1))
    }
}

fn until_char(text: &str, c: char) -> &str {
    StrSplit::new(text, c).next().expect("has some")
}

#[test]
fn until_char_works() {
    let haystack = "hello world";
    assert_eq!(until_char(haystack, 'o'), "hell");
}

#[test]
fn it_works() {
    let haystack = "a b c d e";
    let letters: Vec<_> = StrSplit::new(haystack, " ").collect();
    assert_eq!(letters, vec!["a", "b", "c", "d", "e"]);
}

#[test]
fn tail() {
    let haystack = "a b c d ";
    let letters: Vec<_> = StrSplit::new(haystack, " ").collect();
    assert_eq!(letters, vec!["a", "b", "c", "d", ""]);
}
#[test]
fn tail2() {
    let haystack = "a b c d  ";
    let letters: Vec<_> = StrSplit::new(haystack, " ").collect();
    assert_eq!(letters, vec!["a", "b", "c", "d", "", ""]);
}
