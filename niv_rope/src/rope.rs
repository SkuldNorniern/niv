// A simplified, readable Rope implementation built on a red-black tree of gap-buffer leaves.
// Uses constants instead of hard-coded values, minimizes boilerplate, and keeps debug-only
// printing behind cfg(test).

use crate::rbt_chunk::RBError;

// Basic types and constants
pub type NodeId = u64;
pub const NIL: NodeId = u64::MAX;
pub const LEAF_CAPACITY: usize = 2048; // maximum bytes in a leaf buffer
pub const LEAF_USABLE: usize = (LEAF_CAPACITY * 80) / 100; // 80% of capacity (1638 bytes) for actual content

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq)]
enum Color { Red, Black }

#[derive(Debug, Clone, PartialEq)]
struct Leaf {
    buf: [u8; LEAF_CAPACITY],
    gap_lo: u16,
    gap_hi: u16,
    nl_idx: Vec<u16>,
}

impl Leaf {
    fn new() -> Self {
        Self { buf: [0; LEAF_CAPACITY], gap_lo: 0, gap_hi: LEAF_CAPACITY as u16, nl_idx: Vec::new() }
    }

    #[inline]
    fn gap_size(&self) -> usize { self.gap_hi as usize - self.gap_lo as usize }

    #[inline]
    fn byte_len(&self) -> usize { self.gap_lo as usize + (LEAF_CAPACITY - self.gap_hi as usize) }

    #[inline]
    fn move_gap_to(&mut self, off: usize) {
        let gl = self.gap_lo as usize;
        let gh = self.gap_hi as usize;
        if off < gl {
            let n = gl - off;
            self.buf.copy_within(off..gl, gh - n);
            self.gap_lo = off as u16;
            self.gap_hi = (gh - n) as u16;
        } else if off > gl {
            let n = off - gl;
            self.buf.copy_within(gh..gh + n, gl);
            self.gap_lo = off as u16;
            self.gap_hi = (gh + n) as u16;
        } else {
            // already at off
        }
    }

    #[inline]
    fn partition_point_nl(&self, at: usize) -> usize {
        self.nl_idx.partition_point(|&p| (p as usize) < at)
    }

    fn insert_newline_indices(&mut self, at: usize, data: &[u8]) {
        if data.is_empty() { return; }
        let mut new_positions: Vec<u16> = Vec::new();
        for (i, b) in data.iter().enumerate() {
            if *b == b'\n' {
                let pos = at + i;
                if pos <= u16::MAX as usize { new_positions.push(pos as u16); }
            }
        }
        if new_positions.is_empty() { return; }
        let insert_at = self.partition_point_nl(at);
        let added = data.len();
        for p in &mut self.nl_idx[insert_at..] { *p = (*p as usize + added) as u16; }
        self.nl_idx.splice(insert_at..insert_at, new_positions.into_iter());
    }

    fn remove_newline_indices_in_range(&mut self, start: usize, end: usize) {
        if start >= end { return; }
        let start_i = self.partition_point_nl(start);
        let end_i = self.partition_point_nl(end);
        self.nl_idx.drain(start_i..end_i);
        let removed = end - start;
        for p in &mut self.nl_idx[start_i..] { *p = (*p as usize - removed) as u16; }
    }

    fn insert(&mut self, off: usize, data: &[u8]) -> Result<usize, RBError> {
        if off > self.byte_len() { return Err(RBError::InvalidOffset); }
        if data.is_empty() { return Ok(0); }
        let avail = self.gap_size();
        if avail == 0 { return Err(RBError::InsufficientSpace); }
        let to_copy = core::cmp::min(avail, data.len());
        self.move_gap_to(off);
        let gl = self.gap_lo as usize;
        self.buf[gl..gl + to_copy].copy_from_slice(&data[..to_copy]);
        self.gap_lo = (gl + to_copy) as u16;
        self.insert_newline_indices(off, &data[..to_copy]);
        Ok(to_copy)
    }

    fn delete(&mut self, off: usize, len: usize) -> Result<usize, RBError> {
        let cur_len = self.byte_len();
        if off > cur_len { return Err(RBError::InvalidOffset); }
        if len == 0 { return Ok(0); }
        let end = core::cmp::min(cur_len, off + len);
        let actual = end - off;
        if actual == 0 { return Ok(0); }
        self.move_gap_to(off);
        self.gap_hi = (self.gap_hi as usize + actual) as u16;
        self.remove_newline_indices_in_range(off, off + actual);
        Ok(actual)
    }

    fn read_into(&self, off: usize, out: &mut [u8]) -> Result<usize, RBError> {
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
            Ok(want)
        } else {
            let phys = off + (gh - gl);
            out[..want].copy_from_slice(&self.buf[phys..phys + want]);
            Ok(want)
        }
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

#[derive(Debug, Clone, PartialEq)]
enum Payload { Leaf(Leaf) }

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
pub struct Rope {
    root: NodeId,
    nodes: Vec<Node>,
}

impl Rope {
    pub fn new() -> Self { Self { root: NIL, nodes: Vec::new() } }

    pub fn len(&self) -> usize {
        let mut total = 0usize;
        for n in &self.nodes {
            match &n.payload { Payload::Leaf(l) => { total += l.byte_len(); } }
        }
        total
    }

    pub fn total_lines(&self) -> usize {
        if self.root == NIL { 0 } else { self.nodes[self.root as usize].sub_lines as usize }
    }

    // Recompute this node's subtree aggregates from its children and own leaf
    #[inline]
    fn recompute_node_aggregates(&mut self, node_id: NodeId) {
        if node_id == NIL { return; }
        let idx = node_id as usize;
        let left = self.nodes[idx].left;
        let right = self.nodes[idx].right;
        let left_bytes = if left == NIL { 0 } else { self.nodes[left as usize].sub_bytes as usize };
        let right_bytes = if right == NIL { 0 } else { self.nodes[right as usize].sub_bytes as usize };
        let left_lines = if left == NIL { 0 } else { self.nodes[left as usize].sub_lines as usize };
        let right_lines = if right == NIL { 0 } else { self.nodes[right as usize].sub_lines as usize };
        let own = match &self.nodes[idx].payload { Payload::Leaf(l) => l.byte_len() };
        let own_lines = match &self.nodes[idx].payload { Payload::Leaf(l) => l.nl_idx.len() };
        self.nodes[idx].sub_bytes = (left_bytes + own + right_bytes) as u64;
        self.nodes[idx].sub_lines = (left_lines + own_lines + right_lines) as u64;
    }

    // Update aggregates from this node up to the root
    #[inline]
    fn update_ancestors(&mut self, node_id: NodeId) {
        let mut cur = node_id;
        while cur != NIL {
            self.recompute_node_aggregates(cur);
            cur = self.nodes[cur as usize].parent;
        }
    }

    pub fn build_from_bytes(&mut self, data: &[u8]) -> Result<usize, RBError> {
        self.root = NIL; self.nodes.clear();
        let mut inserted_total = 0usize;
        let mut key: u64 = 0;
        while inserted_total < data.len() {
            let remaining = data.len() - inserted_total;
            let take = if remaining > LEAF_USABLE { LEAF_USABLE } else { remaining };
            let new_id = self.insert_with_id(key)?;
            key = key.saturating_add(1);
            let leaf = match &mut self.nodes[new_id as usize].payload { Payload::Leaf(l) => l };
            let wrote = leaf.insert(0, &data[inserted_total..inserted_total + take])?;
            inserted_total += wrote;
            // Update aggregates for this new leaf up to root
            self.update_ancestors(new_id);
        }
        Ok(inserted_total)
    }

    pub fn read_bytes_global(&self, off: usize, out: &mut [u8]) -> Result<usize, RBError> {
        let mut written = 0usize;
        let mut cur = self.min_node(self.root);
        let mut skip = off;
        while cur != NIL && written < out.len() {
            let l = match &self.nodes[cur as usize].payload { Payload::Leaf(l) => l };
            let ll = l.byte_len();
            if skip >= ll { skip -= ll; }
            else {
                let want = core::cmp::min(out.len() - written, ll - skip);
                let w = l.read_into(skip, &mut out[written..written + want])?;
                written += w; skip = 0;
            }
            cur = self.successor(cur);
        }
        Ok(written)
    }

    pub fn find_first(&self, needle: &[u8]) -> Option<usize> {
        if needle.is_empty() { return Some(0); }
        let mut all: Vec<u8> = Vec::new();
        let mut cur = self.min_node(self.root);
        while cur != NIL {
            let l = match &self.nodes[cur as usize].payload { Payload::Leaf(l) => l };
            let mut tmp = vec![0u8; l.byte_len()];
            if l.read_into(0, &mut tmp).ok()? == tmp.len() { all.extend_from_slice(&tmp); }
            cur = self.successor(cur);
        }
        if all.len() < needle.len() { return None; }
        let last = all.len() - needle.len();
        let mut i = 0usize;
        while i <= last {
            if &all[i..i + needle.len()] == needle { return Some(i); }
            i += 1;
        }
        None
    }

    pub fn replace_first(&mut self, needle: &[u8], replacement: &[u8]) -> Result<usize, RBError> {
        if needle.is_empty() { return Ok(0); }
        let Some(mut global_off) = self.find_first(needle) else { return Ok(0); };
        let mut cur = self.min_node(self.root);
        while cur != NIL {
            let idx = cur as usize;
            let replaced = match &mut self.nodes[idx].payload {
                Payload::Leaf(l) => {
                    let ll = l.byte_len();
                    if global_off >= ll { global_off -= ll; false } else {
                        // Check if replacement fits in current leaf
                        let available = LEAF_CAPACITY - ll;
                        if replacement.len() <= available {
                            // Simple replacement within leaf capacity
                            let del = l.delete(global_off, needle.len())?;
                            if del != needle.len() { return Ok(0); }
                            let ins = l.insert(global_off, replacement)?;
                            if ins != replacement.len() { return Ok(0); }
                            // Update subtree aggregates from this node upward
                            self.update_ancestors(cur);
                            true
                        } else {
                            // Need to restructure tree - split leaf
                            self.restructure_leaf_for_replacement(cur, global_off, needle, replacement)?
                        }
                    }
                }
            };
            if replaced { return Ok(replacement.len()); }
            cur = self.successor(cur);
        }
        Ok(0)
    }

    fn restructure_leaf_for_replacement(&mut self, leaf_id: NodeId, offset: usize, needle: &[u8], replacement: &[u8]) -> Result<bool, RBError> {
        // FEAT:TODO: Missing tree restructuring for leaf overflow
        // This method should:
        // 1. Split the overflowing leaf into multiple leaves
        // 2. Redistribute content to fit within capacity limits  
        // 3. Update tree structure (parent/child relationships)
        // 4. Maintain Red-Black tree properties (colors, balance)
        // 5. Update metadata (sub_bytes, sub_lines counts)
        // 6. Handle cross-leaf content coordination
        !todo!("Tree restructuring not yet implemented - leaf overflow at offset {} with {} byte replacement", offset, replacement.len());
    }

    // Tree operations (BST + RB insert/rotations)
    pub fn insert(&mut self, key: u64) -> Result<(), RBError> { let _ = self.insert_with_id(key)?; Ok(()) }

    fn insert_with_id(&mut self, key: u64) -> Result<NodeId, RBError> {
        let new_node = Node::new(key);
        let new_id = self.nodes.len() as NodeId;
        if new_id == NIL { return Err(RBError::TreeFull); }
        self.nodes.push(new_node);
        if self.root == NIL { self.root = new_id; self.nodes[new_id as usize].color = Color::Black; return Ok(new_id); }
        let mut cur = self.root; let mut parent = NIL; let mut is_left = false;
        while cur != NIL {
            parent = cur;
            if key < self.nodes[cur as usize].key { cur = self.nodes[cur as usize].left; is_left = true; }
            else { cur = self.nodes[cur as usize].right; is_left = false; }
        }
        self.nodes[new_id as usize].parent = parent;
        if is_left { self.nodes[parent as usize].left = new_id; } else { self.nodes[parent as usize].right = new_id; }
        self.insert_fixup(new_id);
        Ok(new_id)
    }

    fn insert_fixup(&mut self, mut n: NodeId) {
        while n != self.root && self.nodes[self.nodes[n as usize].parent as usize].color == Color::Red {
            let p = self.nodes[n as usize].parent; let g = self.nodes[p as usize].parent;
            if p == self.nodes[g as usize].left {
                let u = self.nodes[g as usize].right;
                if u != NIL && self.nodes[u as usize].color == Color::Red {
                    self.nodes[p as usize].color = Color::Black; self.nodes[u as usize].color = Color::Black; self.nodes[g as usize].color = Color::Red; n = g;
                } else {
                    if n == self.nodes[p as usize].right { self.left_rotate(p); n = p; }
                    let p2 = self.nodes[n as usize].parent; let g2 = self.nodes[p2 as usize].parent;
                    self.nodes[p2 as usize].color = Color::Black; self.nodes[g2 as usize].color = Color::Red; self.right_rotate(g2);
                }
            } else {
                let u = self.nodes[g as usize].left;
                if u != NIL && self.nodes[u as usize].color == Color::Red {
                    self.nodes[p as usize].color = Color::Black; self.nodes[u as usize].color = Color::Black; self.nodes[g as usize].color = Color::Red; n = g;
                } else {
                    if n == self.nodes[p as usize].left { self.right_rotate(p); n = p; }
                    let p2 = self.nodes[n as usize].parent; let g2 = self.nodes[p2 as usize].parent;
                    self.nodes[p2 as usize].color = Color::Black; self.nodes[g2 as usize].color = Color::Red; self.left_rotate(g2);
                }
            }
        }
        self.nodes[self.root as usize].color = Color::Black;
        // After structural changes, update aggregates from the affected node upward
        self.update_ancestors(n);
    }

    fn left_rotate(&mut self, x: NodeId) {
        let y = self.nodes[x as usize].right; let y_left = self.nodes[y as usize].left; let x_parent = self.nodes[x as usize].parent;
        self.nodes[x as usize].right = y_left; if y_left != NIL { self.nodes[y_left as usize].parent = x; }
        self.nodes[y as usize].parent = x_parent;
        if x == self.root { self.root = y; } else if x == self.nodes[x_parent as usize].left { self.nodes[x_parent as usize].left = y; } else { self.nodes[x_parent as usize].right = y; }
        self.nodes[y as usize].left = x; self.nodes[x as usize].parent = y;
        // Recompute aggregates bottom-up for x then y, then up to root from y
        self.recompute_node_aggregates(x);
        self.recompute_node_aggregates(y);
        self.update_ancestors(self.nodes[y as usize].parent);
    }

    fn right_rotate(&mut self, y: NodeId) {
        let x = self.nodes[y as usize].left; let x_right = self.nodes[x as usize].right; let y_parent = self.nodes[y as usize].parent;
        self.nodes[y as usize].left = x_right; if x_right != NIL { self.nodes[x_right as usize].parent = y; }
        self.nodes[x as usize].parent = y_parent;
        if y == self.root { self.root = x; } else if y == self.nodes[y_parent as usize].right { self.nodes[y_parent as usize].right = x; } else { self.nodes[y_parent as usize].left = x; }
        self.nodes[x as usize].right = y; self.nodes[y as usize].parent = x;
        // Recompute aggregates bottom-up for y then x, then up to root from x
        self.recompute_node_aggregates(y);
        self.recompute_node_aggregates(x);
        self.update_ancestors(self.nodes[x as usize].parent);
    }

    fn min_node(&self, mut n: NodeId) -> NodeId { if n == NIL { return NIL; } while self.nodes[n as usize].left != NIL { n = self.nodes[n as usize].left; } n }

    fn successor(&self, mut n: NodeId) -> NodeId {
        if n == NIL { return NIL; }
        let r = self.nodes[n as usize].right; if r != NIL { return self.min_node(r); }
        let mut p = self.nodes[n as usize].parent; while p != NIL && n == self.nodes[p as usize].right { n = p; p = self.nodes[p as usize].parent; } p
    }

    // Debug visualization (tests only)
    #[cfg(test)]
    pub fn visualize(&self) {
        if self.root == NIL { println!("<empty rope>"); return; }
        self.visualize_node(self.root, 0);
    }

    #[cfg(test)]
    fn visualize_node(&self, node_id: NodeId, depth: usize) {
        if node_id == NIL { return; }
        let node = &self.nodes[node_id as usize];
        let indent = "  ".repeat(depth);
        let color = match node.color { Color::Red => 'R', Color::Black => 'B' };
        match &node.payload {
            Payload::Leaf(l) => println!("{}[{}] key={} Leaf(bytes={}, lines={})", indent, color, node.key, l.byte_len(), l.nl_idx.len()),
        }
        if node.left != NIL { self.visualize_node(node.left, depth + 1); }
        if node.right != NIL { self.visualize_node(node.right, depth + 1); }
    }

    // FEAT:TODO: Missing advanced rope operations
    // 1. delete_range(start, end) - Remove text range with tree rebalancing
    // 2. insert_at(offset, text) - Insert text at specific offset
    // 3. undo() / redo() - History management for text operations
    // 4. optimize() - Rebalance tree for better performance
    // 5. merge_leaves() - Combine underutilized leaves
    // 6. split_leaf_at(offset) - Split leaf at specific position
    // 7. get_line_info(offset) - Get line number and column for offset
    // 8. find_all(needle) - Find all occurrences of text
    // 9. replace_all(needle, replacement) - Replace all occurrences
    // 10. copy_range(start, end) - Copy text range to new rope
}

impl<'a> Iterator for RopeSlice<'a> {
    type Item = &'a [u8];

    fn next(&mut self) -> Option<Self::Item> {
        if self.current_offset >= self.end {
            return None;
        }

        // Find the leaf containing current_offset
        let mut node = self.current_node;
        let mut offset_in_rope = 0;

        // Navigate to the correct leaf
        while node != NIL {
            let node_idx = node as usize;
            let left_bytes = if self.rope.nodes[node_idx].left != NIL {
                self.rope.nodes[self.rope.nodes[node_idx].left as usize].sub_bytes as usize
            } else {
                0
            };

            if offset_in_rope + left_bytes > self.current_offset {
                // Go left
                node = self.rope.nodes[node_idx].left;
            } else {
                offset_in_rope += left_bytes;
                let own_bytes = match &self.rope.nodes[node_idx].payload {
                    Payload::Leaf(l) => l.byte_len(),
                };

                if offset_in_rope + own_bytes > self.current_offset {
                    // This is the correct leaf
                    break;
                } else {
                    // Go right
                    offset_in_rope += own_bytes;
                    node = self.rope.nodes[node_idx].right;
                }
            }
        }

        if node == NIL {
            return None;
        }

        // Extract slice from this leaf - zero-copy approach
        let Payload::Leaf(leaf) = &self.rope.nodes[node as usize].payload;
        let leaf_start = (self.current_offset - offset_in_rope).min(leaf.byte_len());
        let leaf_end = (self.end - offset_in_rope).min(leaf.byte_len());

        if leaf_start >= leaf_end {
            // Move to next leaf
            self.current_node = self.rope.successor(node);
            return self.next();
        }

        let slice_len = leaf_end - leaf_start;
        let gl = leaf.gap_lo();
        let gh = leaf.gap_hi();

        // Calculate the actual slice from the buffer, accounting for the gap
        let slice_start = if leaf_start < gl {
            leaf_start
        } else {
            leaf_start + (gh - gl)
        };

        let slice_end = if leaf_end <= gl {
            leaf_end
        } else {
            leaf_end + (gh - gl)
        };

        self.current_offset += slice_len;

        // Update current_node for next iteration
        if self.current_offset >= leaf_end + offset_in_rope {
            self.current_node = self.rope.successor(node);
        }

        // Return the slice directly from the buffer - this is zero-copy
        Some(&leaf.buf()[slice_start..slice_end])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rope_build_len_read() {
        let mut rope = Rope::new();
        let pattern = b"Hello World, I need editor\n";
        let mut buf: Vec<u8> = Vec::new();
        while buf.len() < LEAF_CAPACITY * 3 + 123 { buf.extend_from_slice(pattern); }
        let wrote = rope.build_from_bytes(&buf).expect("build");
        assert_eq!(wrote, buf.len());
        assert_eq!(rope.len(), buf.len());
        let mut out = vec![0u8; 96];
        let r = rope.read_bytes_global(0, &mut out).expect("read");
        assert_eq!(&out[..r], &buf[..r]);
    }

    #[test]
    fn rope_find_replace_same_len() {
        let mut rope = Rope::new();
        let data = b"void draw(void){ puts(\"draw\"); }\nvoid tick(float dt){}\n";
        let _ = rope.build_from_bytes(data).expect("build");
        let pos = rope.find_first(b"draw(");
        assert!(pos.is_some());
        let ok = rope.replace_first_same_len(b"draw(", b"show(").expect("replace");
        assert!(ok);
        let mut all = vec![0u8; rope.len()];
        let _ = rope.read_bytes_global(0, &mut all).expect("read all");
        assert!(std::str::from_utf8(&all).unwrap_or("").contains("show("));
    }

    #[test]
    fn rope_debug_visualize() {
        let mut rope = Rope::new();
        let mut long = vec![0u8; LEAF_CAPACITY * 5];
        for i in 0..long.len() { long[i] = b'A' + (i % 23) as u8; }
        let _ = rope.build_from_bytes(&long).expect("build");
        // This just exercises the code path; output is manual-debug only
        rope.visualize();
    }

    #[test]
    fn rope_total_lines_basic() {
        let mut rope = Rope::new();
        let data = b"a\nb\nc\n"; // 3 newlines
        let wrote = rope.build_from_bytes(data).expect("build");
        assert_eq!(wrote, data.len());
        assert_eq!(rope.len(), data.len());
        assert_eq!(rope.total_lines(), 3);
    }

    #[test]
    fn rope_read_global_across_leaves() {
        let mut rope = Rope::new();
        // Build content spanning multiple leaves
        let line = b"0123456789abcdefghijklmnopqrstuvwxyz\n"; // 37 bytes including newline
        let mut data: Vec<u8> = Vec::new();
        while data.len() < LEAF_USABLE * 2 + 50 { data.extend_from_slice(line); }
        let _ = rope.build_from_bytes(&data).expect("build");

        // Read across the boundary between leaf 0 and 1
        let start = LEAF_USABLE.saturating_sub(20);
        let want = 80usize;
        let mut out = vec![0u8; want];
        let r = rope.read_bytes_global(start, &mut out).expect("read");
        assert_eq!(&out[..r], &data[start..start + r]);
    }

    #[test]
    fn rope_replace_var_len_updates_metadata() {
        let mut rope = Rope::new();
        // Two lines initially (2 newlines)
        let data = b"abc\ndef\n"; // len 8, lines 2
        let _ = rope.build_from_bytes(data).expect("build");
        assert_eq!(rope.len(), 8);
        assert_eq!(rope.total_lines(), 2);

        // Replace "def" (3) with "d\ne\nf" (5), adding 2 newlines
        let replaced = rope.replace_first(b"def", b"d\ne\nf").expect("replace");
        assert_eq!(replaced, 5);
        // Length increases by +2
        assert_eq!(rope.len(), 10);
        // Newline count increases by +2 (now 4)
        assert_eq!(rope.total_lines(), 4);

        // Verify content around replacement
        let mut all = vec![0u8; rope.len()];
        let _ = rope.read_bytes_global(0, &mut all).expect("read all");
        let s = std::str::from_utf8(&all).unwrap_or("");
        assert!(s.contains("abc\nd\ne\nf\n"));
    }

    #[test]
    fn rope_metadata_consistency_large_build() {
        let mut rope = Rope::new();
        let mut data: Vec<u8> = Vec::new();
        // Build 5000 lines
        for i in 0..5000 {
            let mut line = format!("line_{i}\n").into_bytes();
            data.append(&mut line);
        }
        let _ = rope.build_from_bytes(&data).expect("build");
        let expected_newlines = data.iter().filter(|&&b| b == b'\n').count();
        assert_eq!(rope.len(), data.len());
        assert_eq!(rope.total_lines(), expected_newlines);

        // Spot-check a mid-range slice matches
        let start = data.len() / 3;
        let mut buf = vec![0u8; 123];
        let r = rope.read_bytes_global(start, &mut buf).expect("read");
        assert_eq!(&buf[..r], &data[start..start + r]);
    }

    #[test]
    fn rope_slice_iterator_zero_copy() {
        let mut rope = Rope::new();
        let data = b"Hello World\nThis is a test\nWith multiple lines";
        let _ = rope.build_from_bytes(data).expect("build");

        // Test full slice iteration
        let slice = rope.slice(0, data.len());
        let mut collected = Vec::new();
        for chunk in slice {
            collected.extend_from_slice(chunk);
        }
        assert_eq!(collected, data);

        // Test partial slice iteration
        let slice = rope.slice(6, 18); // "World\nThis is"
        let mut collected = Vec::new();
        for chunk in slice {
            collected.extend_from_slice(chunk);
        }
        assert_eq!(collected, &data[6..18]);
    }

    #[test]
    fn rope_slice_iterator_single_leaf() {
        let mut rope = Rope::new();
        let data = b"Hello World\nThis is a test\nWith multiple lines";
        let _ = rope.build_from_bytes(data).expect("build");

        // Test partial slice within a single leaf
        let slice = rope.slice(6, 25); // "World\nThis is a t"
        let mut collected = Vec::new();
        for chunk in slice {
            collected.extend_from_slice(chunk);
        }
        assert_eq!(collected, &data[6..25]);

        // Should get exactly one chunk since it's within a single leaf
        let slice = rope.slice(6, 25);
        let chunk_count = slice.count();
        assert_eq!(chunk_count, 1, "Expected single chunk within leaf, got {}", chunk_count);
    }

    #[test]
    fn rope_slice_iterator_bounds() {
        let mut rope = Rope::new();
        let data = b"Short text";
        let _ = rope.build_from_bytes(data).expect("build");

        // Test out of bounds handling
        let slice = rope.slice(5, 20); // Should clamp to valid range
        let mut collected = Vec::new();
        for chunk in slice {
            collected.extend_from_slice(chunk);
        }
        assert_eq!(collected, &data[5..]); // Should get from offset 5 to end

        // Test empty slice
        let slice = rope.slice(5, 5);
        let chunk_count = slice.count();
        assert_eq!(chunk_count, 0);
    }
}
