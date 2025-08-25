type NodeId = u64;
const NIL: NodeId = u64::MAX;
const LEAF_MAX_SIZE: usize = 2048;

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq)]
enum Color {
    Red,
    Black,
}

#[derive(Debug, Clone, Copy)]
pub enum RBError {
    TreeFull,
    InvalidOffset,
    InsufficientSpace,
}
impl std::fmt::Display for RBError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RBError::TreeFull => write!(f, "Tree is full"),
            RBError::InvalidOffset => write!(f, "Invalid offset"),
            RBError::InsufficientSpace => write!(f, "Insufficient space"),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
enum Payload {
    Leaf(Leaf),
    Branch,
}

#[derive(Debug, Clone, PartialEq)]
struct Leaf {
    buf: [u8; LEAF_MAX_SIZE],
    gap_lo: u16,
    gap_hi: u16,
    
    nl_idx: Vec<u16>,
}
impl Leaf {
    fn new() -> Self {
        Self {
            buf: [0; LEAF_MAX_SIZE],
            gap_lo: 0,
            gap_hi: LEAF_MAX_SIZE as u16,
            nl_idx: Vec::new(),
        }
    }
    #[inline]
    fn move_gap_to(&mut self, off: usize) {
        #[cfg(debug_assertions)]
        self.dbg_state("before move_gap_to");
        #[cfg(debug_assertions)]
        println!("move_gap_to: off={}", off);
        let gl = self.gap_lo as usize;
        let gh = self.gap_hi as usize;

        if off < gl {
            // Move LEFT bytes [off .. gl) to the RIGHT edge of the gap: [gh - n .. gh)
            let n = gl - off;
            // use copy_within (memmove semantics) or ptr::copy for overlaps
            self.buf.copy_within(off..gl, gh - n);
            self.gap_lo = off as u16;
            self.gap_hi = (gh - n) as u16;

        } else if off > gl {
            // Move RIGHT bytes [gh .. gh + n) to the LEFT edge of the gap: [gl .. gl + n)
            let n = off - gl;
            self.buf.copy_within(gh..gh + n, gl);
            self.gap_lo = off as u16;
            self.gap_hi = (gh + n) as u16;

        } else {
            // already at off
        }

        // (Optional debug) poison-fill the gap so you catch accidental reads:
        #[cfg(debug_assertions)]
        for i in self.gap_lo as usize .. self.gap_hi as usize {
            self.buf[i] = 0xDD;
        }
        #[cfg(debug_assertions)]
        self.dbg_state("after move_gap_to");
    }

    #[inline]
    fn gap_size(&self) -> usize {
        self.gap_hi as usize - self.gap_lo as usize
    }

    #[inline]
    fn byte_len(&self) -> usize {
        self.gap_lo as usize + (LEAF_MAX_SIZE - self.gap_hi as usize) 
    }

    #[inline]
    fn partition_point_nl(&self, at: usize) -> usize {
        // Stable since 1.52
        self.nl_idx.partition_point(|&p| (p as usize) < at)
    }

    fn insert_newline_indices(&mut self, at: usize, data: &[u8]) {
        if data.is_empty() { return; }
        let mut new_positions: Vec<u16> = Vec::new();
        for (i, b) in data.iter().enumerate() {
            if *b == b'\n' {
                let pos = at + i;
                if pos <= u16::MAX as usize {
                    new_positions.push(pos as u16);
                }
            }
        }
        if new_positions.is_empty() { return; }
        let insert_at = self.partition_point_nl(at);
        // shift existing >= at by added count
        let added = data.len();
        for p in &mut self.nl_idx[insert_at..] {
            *p = (*p as usize + added) as u16;
        }
        // splice in sorted
        self.nl_idx.splice(insert_at..insert_at, new_positions.into_iter());
    }

    fn remove_newline_indices_in_range(&mut self, start: usize, end: usize) {
        if start >= end { return; }
        let start_i = self.partition_point_nl(start);
        let end_i = self.partition_point_nl(end);
        self.nl_idx.drain(start_i..end_i);
        let removed = end - start;
        for p in &mut self.nl_idx[start_i..] {
            *p = (*p as usize - removed) as u16;
        }
    }

    pub fn insert(&mut self, off: usize, data: &[u8]) -> Result<usize, RBError> {
        #[cfg(debug_assertions)]
        println!("insert: off={}, len={}", off, data.len());
        #[cfg(debug_assertions)]
        self.dbg_state("before insert");
        if off > self.byte_len() { return Err(RBError::InvalidOffset); }
        if data.is_empty() { return Ok(0); }
        let avail = self.gap_size();
        if avail == 0 { return Err(RBError::InsufficientSpace); }
        let to_copy = core::cmp::min(avail, data.len());
        self.move_gap_to(off);
        let gl = self.gap_lo as usize;
        self.buf[gl .. gl + to_copy].copy_from_slice(&data[..to_copy]);
        self.gap_lo = (gl + to_copy) as u16;

        self.insert_newline_indices(off, &data[..to_copy]);
        #[cfg(debug_assertions)]
        self.dbg_state("after insert");
        Ok(to_copy)
    }

    pub fn delete(&mut self, off: usize, len: usize) -> Result<usize, RBError> {
        #[cfg(debug_assertions)]
        println!("delete: off={}, len={}", off, len);
        #[cfg(debug_assertions)]
        self.dbg_state("before delete");
        let cur_len = self.byte_len();
        if off > cur_len { return Err(RBError::InvalidOffset); }
        if len == 0 { return Ok(0); }
        let end = core::cmp::min(cur_len, off + len);
        let actual = end - off;
        if actual == 0 { return Ok(0); }
        self.move_gap_to(off);
        // Expand gap to cover [off, off+actual)
        self.gap_hi = (self.gap_hi as usize + actual) as u16;
        self.remove_newline_indices_in_range(off, off + actual);
        #[cfg(debug_assertions)]
        self.dbg_state("after delete");
        Ok(actual)
    }

    pub fn read_into(&self, off: usize, out: &mut [u8]) -> Result<usize, RBError> {
        #[cfg(debug_assertions)]
        println!("read_into: off={}, cap={}", off, out.len());
        #[cfg(debug_assertions)]
        self.dbg_state("before read_into");
        let cur_len = self.byte_len();
        if off > cur_len { return Err(RBError::InvalidOffset); }
        let want = core::cmp::min(out.len(), cur_len - off);
        if want == 0 { return Ok(0); }
        let gl = self.gap_lo as usize;
        let gh = self.gap_hi as usize;
        if off < gl {
            let left = core::cmp::min(want, gl - off);
            out[..left].copy_from_slice(&self.buf[off..off + left]);
            let remain = want - left;
            if remain > 0 {
                let src = gh;
                out[left..left + remain].copy_from_slice(&self.buf[src..src + remain]);
            }
            #[cfg(debug_assertions)]
            println!("read_into: read={} (split)", want);
            Ok(want)
        } else {
            let phys = off + (gh - gl);
            out[..want].copy_from_slice(&self.buf[phys..phys + want]);
            #[cfg(debug_assertions)]
            println!("read_into: read={} (right)", want);
            Ok(want)
        }
    }

    #[cfg(debug_assertions)]
    fn dbg_state(&self, label: &str) {
        let used = self.byte_len();
        let gap = self.gap_size();
        println!(
            "Leaf[{}]: used={}, gap_lo={}, gap_hi={}, gap={}, lines={}",
            label, used, self.gap_lo, self.gap_hi, gap, self.nl_idx.len()
        );
    }
}

#[derive(Debug, Clone, PartialEq)]
struct Node {
    key: u64,
    left: NodeId,
    right: NodeId,
    parent: NodeId,
    color: Color,
    
    sub_bytes: u64,
    sub_lines: u64,

    payload: Payload,
    
}

impl Node {
    fn new(key: u64) -> Self {
        Self {
            key,
            left: NIL,
            right: NIL,
            parent: NIL,
            color: Color::Red,
            sub_bytes: 0,
            sub_lines: 0,
            payload: Payload::Leaf(Leaf::new()),
        }
    }
}

#[derive(Debug, Clone)]
pub struct RBRope {
    root: NodeId,
    nodes: Vec<Node>,
}

impl RBRope {
    pub fn new() -> Self {
        Self {
            root: NIL,
            nodes: Vec::new(),
        }
    }

    pub fn insert(&mut self, key: u64) -> Result<(), RBError> {
        let new_node = Node::new(key);
        let new_id = self.nodes.len() as NodeId;
        if new_id == NIL {
            return Err(RBError::TreeFull);
        }
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
        self.nodes[y as usize].parent = x;
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

    fn ensure_root_leaf(&mut self) -> Result<NodeId, RBError> {
        if self.root == NIL {
            let new_id = self.nodes.len() as NodeId;
            if new_id == NIL { return Err(RBError::TreeFull); }
            self.nodes.push(Node::new(0));
            self.root = new_id;
            self.nodes[new_id as usize].color = Color::Black;
        }
        Ok(self.root)
    }

    pub fn len(&self) -> usize {
        if self.root == NIL { return 0; }
        match &self.nodes[self.root as usize].payload {
            Payload::Leaf(l) => l.byte_len(),
            Payload::Branch => 0,
        }
    }

    pub fn insert_bytes(&mut self, off: usize, data: &[u8]) -> Result<usize, RBError> {
        let root_id = self.ensure_root_leaf()?;
        let node = &mut self.nodes[root_id as usize];
        match &mut node.payload {
            Payload::Leaf(l) => l.insert(off, data),
            Payload::Branch => Ok(0),
        }
    }

    pub fn read_bytes(&self, off: usize, out: &mut [u8]) -> Result<usize, RBError> {
        if self.root == NIL { return Ok(0); }
        match &self.nodes[self.root as usize].payload {
            Payload::Leaf(l) => l.read_into(off, out),
            Payload::Branch => Ok(0),
        }
    }

    #[cfg(test)]
    // Debug method to print tree structure
    pub fn debug_print(&self) {
        if self.root == NIL {
            println!("Empty tree");
            return;
        }

        println!("Tree structure:");
        self.debug_print_node(self.root, 0);
    }

    #[cfg(test)]
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

    #[cfg(test)]
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

    #[cfg(test)]
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

    #[cfg(test)]
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

    #[cfg(test)]
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
        let mut tree = RBRope::new();

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
        let mut tree = RBRope::new();

        // Insert keys to form a tree
        assert!(tree.insert(10).is_ok());
        assert!(tree.insert(20).is_ok());
        assert!(tree.insert(5).is_ok());
        assert!(tree.insert(15).is_ok());
        assert!(tree.insert(25).is_ok());

        // Verify Red-Black tree properties
        assert!(tree.is_valid());
    }

    #[test]
    fn test_leaf_insert_hello_world() {
        let mut leaf = Leaf::new();
        let data = b"Hello World, I need editor";
        let wrote = leaf.insert(0, data).expect("insert failed");
        assert_eq!(wrote, data.len());

        let mut out = vec![0u8; leaf.byte_len()];
        let read = leaf.read_into(0, &mut out).expect("read failed");
        assert_eq!(read, data.len());
        assert_eq!(&out[..read], data);
    }

    #[test]
    fn test_rbrope_long_text_capacity() {
        let mut tree = RBRope::new();

        // Build a long text > LEAF_MAX_SIZE
        let pattern = b"Hello World, I need editor\n";
        let mut long_data: Vec<u8> = Vec::new();
        while long_data.len() < 5000 {
            long_data.extend_from_slice(pattern);
        }

        // Append into the root leaf until capacity is reached
        let mut inserted_total = 0usize;
        while inserted_total < long_data.len() {
            let off = tree.len();
            let wrote = tree
                .insert_bytes(off, &long_data[inserted_total..])
                .expect("insert into root failed");
            if wrote == 0 { break; }
            inserted_total += wrote;
            if tree.len() >= LEAF_MAX_SIZE { break; }
        }

        // We only have one leaf; verify we filled up to capacity
        assert_eq!(tree.len(), LEAF_MAX_SIZE);

        // Read back and compare with the original prefix
        let mut out = vec![0u8; LEAF_MAX_SIZE];
        let read = tree.read_bytes(0, &mut out).expect("read root failed");
        assert_eq!(read, LEAF_MAX_SIZE);
        assert_eq!(&out[..], &long_data[..LEAF_MAX_SIZE]);
    }
}
