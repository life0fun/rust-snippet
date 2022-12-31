// Single Post owns transition States.
// Various Posts has different state handling methods.
//
use std::mem;

// 
// State contains Fixed Common behaviros for blog handling state transition.
// instead of an enum, an full object with state handling fns.
// Easy to add more state that impl the interface. not easy to add more functions.
// make trait Generic over fns for extension, and delegate fns to another trait.
trait BlogState {
    // state transition method only valid when called on the Box holding the type.
    // takes the ownership of Box<BlogState> and return a new Box<State>.
    fn request_review(self: Box<Self>) -> Box<dyn BlogState>;
    fn approve(self: Box<Self>) -> Box<dyn BlogState>;
    // only published state impl this. delegate from Post fn.
    fn content<'a>(&self, post: &'a Post) -> &'a str;
}
// sub traits
trait PostState<ApprovalType> {
  fn content<'a>(&self, post: &'a Post) -> &'a str;
  fn approval(self: Box<Self>, approval_type: ApprovalType) -> Box<dyn PostState<ApprovalType>>;
}
trait ReviewApproval<ApprovalType> {
  fn request_review(self: Box<Self>) -> Box<dyn PostState<ApprovalType>>;
  fn approve(self: Box<Self>) -> Box<dyn PostState<ApprovalType>>;
}
trait ReviewApproalRollout<ApprovalType>: ReviewApproval<ApprovalType> {
  fn rollout(self: Box<Self>) -> Box<dyn PostState<ApprovalType>>;
}
struct InternalApprovalType {}
struct ExternalApprovalType {}
struct InternalApproval {}
struct ExternalApproval {}
// impl ReviewApproval<InternalApprovalType> for InternalApproval {}
// impl ReviewApproalRollout<InternalApprovalType> for InternalApproval {}

// Post owns a state object repr the current state.
// state handler consumes old state and create new state to Post.
pub struct Post {
    //state: Option<Box<dyn BlogState>>,
    state: Box<dyn BlogState>,
    content: String,
}

struct DraftState {}
impl BlogState for DraftState {
    fn request_review(self: Box<Self>) -> Box<dyn BlogState> {
        Box::new(PendingReviewState {})
    }
    fn approve(self: Box<Self>) -> Box<dyn BlogState> {
        self
    }
    fn content<'a>(&self, post: &'a Post) -> &'a str {
        ""
    }
}

struct PendingReviewState {}
impl BlogState for PendingReviewState {
    fn request_review(self: Box<Self>) -> Box<dyn BlogState> {
        self
    }
    fn approve(self: Box<Self>) -> Box<dyn BlogState> {
        Box::new(PublishedState {})
    }
    fn content<'a>(&self, post: &'a Post) -> &'a str {
        ""
    }
}

struct PublishedState {}
impl BlogState for PublishedState {
    fn request_review(self: Box<Self>) -> Box<dyn BlogState> {
        self
    }
    fn approve(self: Box<Self>) -> Box<dyn BlogState> {
        self
    }
    fn content<'a>(&self, post: &'a Post) -> &'a str {
        &post.content
    }
}

impl Post {
    pub fn new() -> Post {
        Post {
            // state: Some(Box::new(DraftState {})),
            state: Box::new(DraftState {}),
            content: String::new(),
        }
    }
    pub fn add_text(&mut self, txt: &str) {
        self.content.push_str(txt);
    }
    pub fn content(&self) -> &str {
        // state is an Option<Box<dyn State>>, as_ref, Option<&Box<dyn State>>
        // deref coerce on &Box<State> => State.content()
        // self.state.as_ref().unwrap().content(self)
        self.state.content(self)
    }

    pub fn request_review(&mut self) {
        // if let Some(s) = self.state.take() {
        //     // take the state from Option, Box<State>
        //     self.state = Some(s.request_review()) // call the state transition method with self=Box<State>
        // }
        let s = std::mem::replace(&mut self.state, Box::new(DraftState {}));
        std::mem::replace(&mut self.state, s.request_review());
    }
    pub fn approve(&mut self) {
        // if let Some(s) = self.state.take() {
        //     self.state = Some(s.approve());
        // }
        let s = std::mem::replace(&mut self.state, Box::new(DraftState {}));
        std::mem::replace(&mut self.state, s.approve());
    }
}

//
// Instead of one blog type owning a state object, mutliple top level blog types
// DraftPost shall not have content() method, compiler check.
struct _DraftPost {
    content: String,
}
struct _PendingReviewPost {
    content: String,
}
struct _Post {
    content: String,
}
// no content method in DraftPost
impl _DraftPost {
    fn add_text(&mut self, text: &str) {
        self.content.push_str(text); // push_str cp str bytes.
    }
    fn request_review(self) -> _PendingReviewPost {
        _PendingReviewPost {
            content: self.content,
        }
    }
}
// no request_review fn for PendingReviewPost.
impl _PendingReviewPost {
    fn approve(self) -> _Post {
        _Post {
            content: self.content,
        }
    }
}
impl _Post {
    pub fn new() -> _DraftPost {
        _DraftPost {
            content: String::new(),
        }
    }
    pub fn content(&self) -> &str {
        &self.content
    }
}
pub fn test() {
    let mut draft_post = _Post::new();
    draft_post.add_text("hello");
    // assert_eq!(post.content(), ""); // no content() method
    let review_post = draft_post.request_review();
    let post = review_post.approve();
    assert_eq!(post.content(), "hello");
}

pub fn main() {
    let mut post = Post::new();
    post.add_text("I ate a salad");
    assert_eq!("", post.content());

    post.request_review();
    assert_eq!("", post.content());

    post.approve();
    assert_eq!("I ate a salad", post.content());
    
    test();
}
