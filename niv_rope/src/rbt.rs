type NodeId = u32;
const NIL: NodeId = u32::MAX;

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq)]
enum Color {
    Red,
    Black,
}

#[derive(Debug, Clone)]
struct Node {
    left: NodeId,
    right: NodeId,
    parent: NodeId,
    color: Color,
    key: u64,
}

impl Node {
    fn new(key: u64) -> Self {
        Self {
            left: NIL,
            right: NIL,
            parent: NIL,
            color: Color::Red,
            key,
        }
    }
}

#[derive(Debug)]
pub struct RBTree {
    root: NodeId,
    nodes: Vec<Node>,
}

impl RBTree {
    pub fn new() -> Self {
        Self {
            root: NIL,
            nodes: Vec::new(),
        }
    }

    pub fn insert(&mut self, key: u64) -> Result<(), &'static str> {
        let new_node = Node::new(key);
        let new_id = self.nodes.len() as NodeId;
        self.nodes.push(new_node);

        if self.root == NIL {
            self.root = new_id;
            self.nodes[new_id as usize].color = Color::Black;
            return Ok(());
        }

        // Perform standard BST insertion
        let mut current = self.root;
        let mut parent = NIL;
        let mut is_left_child = false;

        while current != NIL {
            parent = current;
            let current_key = self.nodes[current as usize].key;

            if key < current_key {
                current = self.nodes[current as usize].left;
                is_left_child = true;
            } else {
                current = self.nodes[current as usize].right;
                is_left_child = false;
            }
        }

        // Link the new node to its parent
        self.nodes[new_id as usize].parent = parent;
        if is_left_child {
            self.nodes[parent as usize].left = new_id;
        } else {
            self.nodes[parent as usize].right = new_id;
        }

        // Fix Red-Black properties
        self.insert_fixup(new_id);
        Ok(())
    }

    fn insert_fixup(&mut self, mut node_id: NodeId) {
        while node_id != self.root
            && self.nodes[self.nodes[node_id as usize].parent as usize].color == Color::Red
        {
            let parent_id = self.nodes[node_id as usize].parent;
            let grandparent_id = self.nodes[parent_id as usize].parent;

            if parent_id == self.nodes[grandparent_id as usize].left {
                let uncle_id = self.nodes[grandparent_id as usize].right;

                if uncle_id != NIL && self.nodes[uncle_id as usize].color == Color::Red {
                    // Case 1: Uncle is red
                    self.nodes[parent_id as usize].color = Color::Black;
                    self.nodes[uncle_id as usize].color = Color::Black;
                    self.nodes[grandparent_id as usize].color = Color::Red;
                    node_id = grandparent_id;
                } else {
                    // Case 2: Uncle is black
                    if node_id == self.nodes[parent_id as usize].right {
                        self.left_rotate(parent_id);
                        node_id = parent_id;
                        // parent_id is now the left child
                    }
                    // Case 3: Uncle is black, node is left child
                    let parent_id = self.nodes[node_id as usize].parent;
                    let grandparent_id = self.nodes[parent_id as usize].parent;
                    self.nodes[parent_id as usize].color = Color::Black;
                    self.nodes[grandparent_id as usize].color = Color::Red;
                    self.right_rotate(grandparent_id);
                }
            } else {
                // Mirror case: parent is right child
                let uncle_id = self.nodes[grandparent_id as usize].left;

                if uncle_id != NIL && self.nodes[uncle_id as usize].color == Color::Red {
                    self.nodes[parent_id as usize].color = Color::Black;
                    self.nodes[uncle_id as usize].color = Color::Black;
                    self.nodes[grandparent_id as usize].color = Color::Red;
                    node_id = grandparent_id;
                } else {
                    if node_id == self.nodes[parent_id as usize].left {
                        self.right_rotate(parent_id);
                        node_id = parent_id;
                    }
                    let parent_id = self.nodes[node_id as usize].parent;
                    let grandparent_id = self.nodes[parent_id as usize].parent;
                    self.nodes[parent_id as usize].color = Color::Black;
                    self.nodes[grandparent_id as usize].color = Color::Red;
                    self.left_rotate(grandparent_id);
                }
            }
        }

        self.nodes[self.root as usize].color = Color::Black;
    }

    fn left_rotate(&mut self, x: NodeId) {
        let y = self.nodes[x as usize].right;
        let y_left = self.nodes[y as usize].left;
        let x_parent = self.nodes[x as usize].parent;

        self.nodes[x as usize].right = y_left;

        if y_left != NIL {
            self.nodes[y_left as usize].parent = x;
        }

        self.nodes[y as usize].parent = x_parent;

        // Only change root if we're rotating around the root
        if x == self.root {
            self.root = y;
        } else if x == self.nodes[x_parent as usize].left {
            self.nodes[x_parent as usize].left = y;
        } else {
            self.nodes[x_parent as usize].right = y;
        }

        self.nodes[y as usize].left = x;
        self.nodes[x as usize].parent = y;
    }

    fn right_rotate(&mut self, y: NodeId) {
        let x = self.nodes[y as usize].left;
        let x_right = self.nodes[x as usize].right;
        let y_parent = self.nodes[y as usize].parent;

        self.nodes[y as usize].left = x_right;

        if x_right != NIL {
            self.nodes[x_right as usize].parent = y;
        }

        self.nodes[x as usize].parent = y_parent;

        // Only change root if we're rotating around the root
        if y == self.root {
            self.root = x;
        } else if y == self.nodes[y_parent as usize].right {
            self.nodes[y_parent as usize].right = x;
        } else {
            self.nodes[y_parent as usize].left = x;
        }

        self.nodes[x as usize].right = y;
        self.nodes[y as usize].parent = y_parent;
    }

    pub fn search(&self, key: u64) -> Option<NodeId> {
        let mut current = self.root;

        while current != NIL {
            let current_key = self.nodes[current as usize].key;
            match key.cmp(&current_key) {
                std::cmp::Ordering::Equal => return Some(current),
                std::cmp::Ordering::Less => current = self.nodes[current as usize].left,
                std::cmp::Ordering::Greater => current = self.nodes[current as usize].right,
            }
        }

        None
    }

    // Debug method to print tree structure
    pub fn debug_print(&self) {
        if self.root == NIL {
            println!("Empty tree");
            return;
        }

        println!("Tree structure:");
        self.debug_print_node(self.root, 0);
    }

    fn debug_print_node(&self, node_id: NodeId, depth: usize) {
        if node_id == NIL {
            return;
        }

        let indent = "  ".repeat(depth);
        let node = &self.nodes[node_id as usize];
        let color_str = if node.color == Color::Red { "R" } else { "B" };

        println!(
            "{}[{}] {} (parent: {}, left: {}, right: {})",
            indent, color_str, node.key, node.parent, node.left, node.right
        );

        if node.left != NIL {
            self.debug_print_node(node.left, depth + 1);
        }
        if node.right != NIL {
            self.debug_print_node(node.right, depth + 1);
        }
    }

    pub fn is_valid(&self) -> bool {
        if self.root == NIL {
            return true;
        }

        // Check property 2: root is black
        if self.nodes[self.root as usize].color != Color::Black {
            return false;
        }

        // Check property 4: no red node has red children
        if !self.check_red_black_property(self.root) {
            return false;
        }

        // Check property 5: all paths have same black height
        let black_height = self.get_black_height(self.root);
        self.check_black_height_property(self.root, black_height, 0)
    }

    fn check_red_black_property(&self, node_id: NodeId) -> bool {
        if node_id == NIL {
            return true;
        }

        let node = &self.nodes[node_id as usize];

        if node.color == Color::Red {
            if node.left != NIL && self.nodes[node.left as usize].color == Color::Red {
                return false;
            }
            if node.right != NIL && self.nodes[node.right as usize].color == Color::Red {
                return false;
            }
        }

        self.check_red_black_property(node.left) && self.check_red_black_property(node.right)
    }

    fn get_black_height(&self, node_id: NodeId) -> u32 {
        if node_id == NIL {
            return 0;
        }

        let mut height = 0;
        let mut current = node_id;

        while current != NIL {
            if self.nodes[current as usize].color == Color::Black {
                height += 1;
            }
            current = self.nodes[current as usize].left;
        }

        height
    }

    fn check_black_height_property(
        &self,
        node_id: NodeId,
        expected_height: u32,
        current_height: u32,
    ) -> bool {
        if node_id == NIL {
            return current_height == expected_height;
        }

        let node = &self.nodes[node_id as usize];
        let new_height = if node.color == Color::Black {
            current_height + 1
        } else {
            current_height
        };

        if node.left == NIL && node.right == NIL {
            return new_height == expected_height;
        }

        self.check_black_height_property(node.left, expected_height, new_height)
            && self.check_black_height_property(node.right, expected_height, new_height)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_insert_and_search() {
        let mut tree = RBTree::new();

        // Insert some keys
        assert!(tree.insert(10).is_ok());
        assert!(tree.insert(20).is_ok());
        assert!(tree.insert(5).is_ok());
        assert!(tree.insert(15).is_ok());

        // Debug: print tree structure
        tree.debug_print();

        // Search for existing keys
        assert!(tree.search(10).is_some());
        assert!(tree.search(20).is_some());
        assert!(tree.search(5).is_some());
        assert!(tree.search(15).is_some());

        // Search for non-existing keys
        assert!(tree.search(25).is_none());
        assert!(tree.search(0).is_none());
    }

    #[test]
    fn test_red_black_properties() {
        let mut tree = RBTree::new();

        // Insert keys to form a tree
        assert!(tree.insert(10).is_ok());
        assert!(tree.insert(20).is_ok());
        assert!(tree.insert(5).is_ok());
        assert!(tree.insert(15).is_ok());
        assert!(tree.insert(25).is_ok());

        // Verify Red-Black tree properties
        assert!(tree.is_valid());
    }
}
