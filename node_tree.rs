use std::cell::RefCell;
use std::fmt;
use std::rc::Rc;

// Each node shared owned by its parent node and children nodes.
// Ref counting a shared object by ref/value.
// Shared Rc disallow mut. To change interior node, use RefCell or Cell.
// Node is not copyable, Ref counting use RefCell to the node and mut.
type NodeRef<T> = Rc<RefCell<_Node<T>>>;

// A node is a value and its neighbor repr by a vec of shared ref to adj nodes.
// to store a ref, need to ensure the ref's lifetime.
// use RC<RefCell<T>> or Rc<Cell<T>>, as the shared ref is ref counted, lifetime is guaranteed.
struct _Node<T> {
    value_: T,
    adjacent_: Vec<NodeRef<T>>,
}
struct _TreeNode<T> {
    // interior mut to left/rite node pointer.
    value: T,
    rank: u32,
    left: NodeRef<T>,
    rite: NodeRef<T>,
}

// A Node contains a NodeRef, a single element tuple struct containing a ref/owning to a Node.
pub struct Node<T>(NodeRef<T>);

impl<T> Node<T> {
    fn new(value: T) -> Node<T> {
        // create a node, wrap into node ref.
        let node = _Node {
            value_: value,
            adjacent_: vec![],
        };
        Node(Rc::new(RefCell::new(node)))
    }

    // Add a Rc clone of the other node to self's adjacent vector.
    // note all refs are immut ref, not mut ref. as interior mutability.
    fn add_adjacent(&self, other: &Node<T>) {
        // RefCell::borrow_mut(&self) give you RefMut<'_, T>
        (self.0.borrow_mut()).adjacent_.push(other.0.clone());
    }
}

pub struct Graph<T> {
    nodes: Vec<Node<T>>,
}

impl<T> Graph<T> {
    fn with_nodes(nodes: Vec<Node<T>>) -> Self {
        Graph { nodes: nodes }
    }
}

pub fn node_graph() {
    // Create some nodes
    let node_1 = Node::new(1);
    let node_2 = Node::new(2);
    let node_3 = Node::new(3);

    // Connect some of the nodes (with directed edges)
    node_1.add_adjacent(&node_2);
    node_1.add_adjacent(&node_3);
    node_2.add_adjacent(&node_1);
    node_3.add_adjacent(&node_1);

    // Add nodes to graph
    let graph = Graph::with_nodes(vec![node_1, node_2, node_3]);

    // Show every node in the graph and list their neighbors
    for node in graph.nodes.iter().map(|n| n.0.borrow()) {
        // extract from Rc<RefCell>
        let value = node.value_;
        let neighbours = node
            .adjacent_
            .iter()
            .map(|n| n.borrow().value_)
            .collect::<Vec<_>>();
        println!("node ({}) is connected to: {:?}", value, neighbours);
    }
}

// BTree and an iter with stack.
// enum binary tree as empty or non empty contains a root tree node.
enum BTree<T> {
    EMPTY,
    NONEMPTY(Box<TreeNode<T>>),
}
// left/rite child is a BTree subtree, instead of TreeNode
// to cover the case of EMPTY.
struct TreeNode<T> {
    value: T,
    rank: u32,
    left: BTree<T>,
    rite: BTree<T>,
}

// a stk to keep in-order traversal state. Returned as the IntoIter type when
// BTree::into_iter() impls IntoIterator trait.
// for x in T => loop { if let Some(e) = T.into_iter().next() { Some(e) }
// Hold ref to hosting TreeNode data in heap.
struct TreeIter<'a, T: 'a> {
    // BTree::IntoIterator::IntoIter
    unvisited: Vec<&'a TreeNode<T>>, // stack of TreeNode refs.
}

use self::BTree::*;
impl<'a, T: 'a> TreeIter<'a, T> {
    // BTree::IntoIterator::IntoIter
    // tree is immut borrow, can be rebind to another node.
    fn stk_left_subtree(&mut self, mut tree: &'a BTree<T>) {
        // while(cur != null) stk.push(cur); cur = cur.left;
        while let NONEMPTY(ref node) = *tree {
            // borrow from matched to local var.
            self.unvisited.push(node);
            tree = &node.left; // rebind to node.left to iterative down.
        }
    }
}

impl<T> BTree<T> {
    fn iter(&self) -> TreeIter<T> {
        let mut tree_iter = TreeIter {
            unvisited: Vec::new(),
        };
        tree_iter.stk_left_subtree(&self);
        tree_iter
    }
}

// Impl IntoIterator for BTree => TreeIter; Impl Iterator for TreeIter.
impl<'a, T: 'a> IntoIterator for &'a BTree<T> {
    type Item = &'a TreeNode<T>;
    type IntoIter = TreeIter<'a, T>;
    // into_iter takes self, not &self. So (&v).into_iter();
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

// Impl Iterator for TreeIter, not for BTree.
impl<'a, T> Iterator for TreeIter<'a, T> {
    type Item = &'a TreeNode<T>;
    fn next(&mut self) -> Option<&'a TreeNode<T>> {
        // match the Option<T> returned from vector pop
        let node = match self.unvisited.pop() {
            None => return None,
            Some(node) => node,
        };
        self.stk_left_subtree(&node.rite);
        Some(node) // ref to TreeNode
    }
}

fn traversal<'a, T>(tree: &'a BTree<T>) -> Vec<&'a TreeNode<T>> {
    let mut v = Vec::new();
    for n in tree {
        v.push(n); // n is ref of a tree node.
    }
    v
}
// fn traversal<'a>(root: &BTree<&'a str>) -> Vec<&'a str> {
//   let mut v = Vec::new();
//   for n in root {
//     v.push(n);
//   }
//   v
// }

fn print_node<T: std::fmt::Display>(root: &BTree<T>) {
    let node = match root {
        // match enum var to pattern
        EMPTY => return,
        NONEMPTY(ref node) => node, // extract matched pattern into var.
    };
    print_node(&node.left);
    println!("{}", node.value);
    print_node(&node.rite);
}

// mov left/rite tree into root.
fn make_node<T>(left: BTree<T>, value: T, rite: BTree<T>) -> BTree<T> {
    let rank = 0;
    NONEMPTY(Box::new(TreeNode {
        left,
        value,
        rank,
        rite,
    }))
}

fn build_tree_own_element() -> BTree<String> {
    let ruby = make_node(EMPTY, "ruby".to_string(), EMPTY);
    let grace = make_node(EMPTY, "grace".to_string(), EMPTY);
    let haijin = make_node(grace, "haijin".to_string(), ruby);
    let dad = make_node(haijin, "Dad".to_string(), EMPTY);
    dad
}
fn build_tree_ref_element<'a>() -> BTree<&'a str> {
    let ruby = make_node(EMPTY, "ruby", EMPTY);
    let grace = make_node(EMPTY, "grace", EMPTY);
    let haijin = make_node(grace, "haijin", ruby);
    let dad = make_node(haijin, "Dad", EMPTY);
    dad
}

pub fn tree_test() {
    let root = build_tree_ref_element();
    print_node(&root);
    let v = traversal(&root)
        .iter() // ret ref to the tree node
        .map(|e| e.value) // map and collect adaptor
        .collect::<Vec<_>>();
    println!("{:?}", v);

    let root = build_tree_own_element();
    // print_node(&root);
    let v = traversal(&root)
        .iter() // ret ref to the tree node
        .map(|e| &e.value)
        .collect::<Vec<_>>();
    println!("{:?}", v);
}

fn main() {
    node_graph();
    tree_test();
}
