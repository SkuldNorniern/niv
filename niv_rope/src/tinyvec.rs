use std::mem::MaybeUninit;

/// A vector that stores up to 2048 elements on the stack
/// Falls back to heap allocation when capacity exceeds 2048
/// This is Just for memory optimization for the Customized rope data structure not for performance.
pub struct TinyVec<T> {
    /// Stack storage for up to 2048 elements
    stack: [MaybeUninit<T>; 2048],
    /// Number of elements currently stored
    len: usize,
    /// Whether we're using heap storage
    using_heap: bool,
    /// Heap storage (only used when using_heap is true)
    heap: Vec<T>,
}

impl<T> TinyVec<T> {
    /// Creates a new empty TinyVec
    pub fn new() -> Self {
        Self {
            stack: std::array::from_fn(|_| MaybeUninit::uninit()),
            len: 0,
            using_heap: false,
            heap: Vec::new(),
        }
    }

    /// Returns the current number of elements
    pub fn len(&self) -> usize {
        self.len
    }

    /// Returns true if the vector is empty
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Returns the current capacity
    pub fn capacity(&self) -> usize {
        if self.using_heap {
            self.heap.capacity()
        } else {
            2048
        }
    }

    /// Pushes an element to the end of the vector
    pub fn push(&mut self, value: T) {
        if self.len >= 2048 && !self.using_heap {
            self.switch_to_heap();
        }

        if self.using_heap {
            self.heap.push(value);
        } else {
            // SAFETY: We know len < 2048 here
            self.stack[self.len].write(value);
        }
        self.len += 1;
    }

    /// Pops an element from the end of the vector
    pub fn pop(&mut self) -> Option<T> {
        if self.len == 0 {
            return None;
        }

        self.len -= 1;
        

        if self.using_heap {
            self.heap.pop()
        } else {
            // SAFETY: We know len < 2048 and the element is initialized
            unsafe {
                Some(self.stack[self.len].assume_init_read())
            }
        }
    }

    /// Returns a reference to the element at the given index
    pub fn get(&self, index: usize) -> Option<&T> {
        if index >= self.len {
            return None;
        }

        let value = if self.using_heap {
            self.heap.get(index)
        } else {
            // SAFETY: We know index < len and the element is initialized
            unsafe {
                Some(self.stack[index].assume_init_ref())
            }
        };

        value
    }

    /// Returns a mutable reference to the element at the given index
    pub fn get_mut(&mut self, index: usize) -> Option<&mut T> {
        if index >= self.len {
            return None;
        }

        let value = if self.using_heap {
            self.heap.get_mut(index)
        } else {
            // SAFETY: We know index < len and the element is initialized
            unsafe {
                Some(self.stack[index].assume_init_mut())
            }
        };

        value
    }

    /// Switches from stack to heap storage
    fn switch_to_heap(&mut self) {
        // Create a new Vec with the elements from stack
        let mut new_heap = Vec::with_capacity(2048 * 2);
        
        // Move elements from stack to heap
        for i in 0..self.len {
            // SAFETY: We know the element is initialized
            unsafe {
                let value = self.stack[i].assume_init_read();
                new_heap.push(value);
            }
        }

        // Clear the stack
        for i in 0..self.len {
            // SAFETY: We know the element is initialized and we're clearing it
            unsafe {
                self.stack[i].assume_init_drop();
            }
        }

        self.heap = new_heap;
        self.using_heap = true;
    }
}

impl<T> Drop for TinyVec<T> {
    fn drop(&mut self) {
        if self.using_heap {
            // Heap elements will be dropped automatically when Vec is dropped
            return;
        }

        // Drop stack elements
        for i in 0..self.len {
            // SAFETY: We know the element is initialized
            unsafe {
                self.stack[i].assume_init_drop();
            }
        }
    }
}

impl<T> Default for TinyVec<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> std::ops::Index<usize> for TinyVec<T> {
    type Output = T;

    fn index(&self, index: usize) -> &Self::Output {
        self.get(index).expect("Index out of bounds")
    }
}

impl<T> std::ops::IndexMut<usize> for TinyVec<T> {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        self.get_mut(index).expect("Index out of bounds")
    }
}

// Only implement Clone when T is Clone
impl<T: Clone> Clone for TinyVec<T> {
    fn clone(&self) -> Self {
        let mut new_vec = TinyVec::new();
        for i in 0..self.len {
            if self.using_heap {
                new_vec.push(self.heap[i].clone());
            } else {
                // SAFETY: We know the element is initialized
                unsafe {
                    let value = self.stack[i].assume_init_ref();
                    new_vec.push(value.clone());
                }
            }
        }
        new_vec
    }
}

// Only implement PartialEq when T is PartialEq
impl<T: PartialEq> PartialEq for TinyVec<T> {
    fn eq(&self, other: &Self) -> bool {
        if self.len != other.len {
            return false;
        }
        
        for i in 0..self.len {
            let self_val = if self.using_heap {
                &self.heap[i]
            } else {
                // SAFETY: We know the element is initialized
                unsafe { self.stack[i].assume_init_ref() }
            };
            
            let other_val = if other.using_heap {
                &other.heap[i]
            } else {
                // SAFETY: We know the element is initialized
                unsafe { other.stack[i].assume_init_ref() }
            };
            
            if self_val != other_val {
                return false;
            }
        }
        true
    }
}

// Only implement Eq when T is Eq
impl<T: Eq> Eq for TinyVec<T> {}

// Only implement Debug when T is Debug
impl<T: std::fmt::Debug> std::fmt::Debug for TinyVec<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_list()
            .entries((0..self.len).map(|i| {
                if self.using_heap {
                    &self.heap[i]
                } else {
                    // SAFETY: We know the element is initialized
                    unsafe { self.stack[i].assume_init_ref() }
                }
            }))
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new() {
        let vec: TinyVec<i32> = TinyVec::new();
        assert_eq!(vec.len(), 0);
        assert!(vec.is_empty());
        assert_eq!(vec.capacity(), 2048);
    }

    #[test]
    fn test_push_pop() {
        let mut vec = TinyVec::new();
        vec.push(1);
        vec.push(2);
        vec.push(3);
        
        assert_eq!(vec.len(), 3);
        assert_eq!(vec.pop(), Some(3));
        assert_eq!(vec.pop(), Some(2));
        assert_eq!(vec.pop(), Some(1));
        assert_eq!(vec.pop(), None);
    }

    #[test]
    fn test_get() {
        let mut vec = TinyVec::new();
        vec.push(42);
        vec.push(100);
        
        assert_eq!(vec.get(0), Some(&42));
        assert_eq!(vec.get(1), Some(&100));
        assert_eq!(vec.get(2), None);
    }

    #[test]
    fn test_index() {
        let mut vec = TinyVec::new();
        vec.push(42);
        vec.push(100);
        
        assert_eq!(vec[0], 42);
        assert_eq!(vec[1], 100);
    }

    #[test]
    fn test_stack_to_heap_transition() {
        let mut vec = TinyVec::new();
        
        // Fill up the stack
        for i in 0..2048 {
            vec.push(i);
        }
        
        assert_eq!(vec.len(), 2048);
        assert_eq!(vec.capacity(), 2048);
        
        // This should trigger heap allocation
        vec.push(2048);
        
        assert_eq!(vec.len(), 2049);
        assert!(vec.capacity() > 2048);
    }

    #[test]
    fn test_u32_operations() {
        let mut vec: TinyVec<u32> = TinyVec::new();
        
        // Test with u32 values
        for i in 0..100 {
            vec.push(i * 2);
        }
        
        assert_eq!(vec.len(), 100);
        assert_eq!(vec[0], 0);
        assert_eq!(vec[50], 100);
        assert_eq!(vec[99], 198);
        
        // Test modification
        vec[25] = 999;
        assert_eq!(vec[25], 999);
        
        // Test iteration-like access
        let mut sum = 0;
        for i in 0..vec.len() {
            sum += vec[i];
        }
        // Sum of 0, 2, 4, ..., 48, 999, 52, ..., 198
        // Original sum: 0 + 2 + 4 + ... + 198 = 9900
        // Modified: 999 - 50 = 949 (since vec[25] was 50, now 999)
        let expected_sum = 9900 + 949;
        assert_eq!(sum, expected_sum);
    }

    #[test]
    fn test_string_operations() {
        let mut vec: TinyVec<String> = TinyVec::new();
        
        // Test with String values
        vec.push("hello".to_string());
        vec.push("world".to_string());
        vec.push("rust".to_string());
        
        assert_eq!(vec.len(), 3);
        assert_eq!(vec[0], "hello");
        assert_eq!(vec[1], "world");
        assert_eq!(vec[2], "rust");
        
        // Test modification
        vec[1] = "modified".to_string();
        assert_eq!(vec[1], "modified");
        
        // Test pop with String
        let popped = vec.pop().unwrap();
        assert_eq!(popped, "rust");
        assert_eq!(vec.len(), 2);
    }

    #[test]
    fn test_boolean_operations() {
        let mut vec: TinyVec<bool> = TinyVec::new();
        
        // Test with boolean values
        for i in 0..100 {
            vec.push(i % 2 == 0);
        }
        
        assert_eq!(vec.len(), 100);
        assert_eq!(vec[0], true);   // 0 % 2 == 0
        assert_eq!(vec[1], false);  // 1 % 2 == 1
        assert_eq!(vec[2], true);   // 2 % 2 == 0
        
        // Test boolean operations
        let true_count = (0..vec.len()).filter(|&i| vec[i]).count();
        assert_eq!(true_count, 50); // Half should be true
    }

    #[test]
    fn test_custom_struct() {
        #[derive(Debug, Clone, PartialEq)]
        struct TestStruct {
            id: u32,
            name: String,
            active: bool,
        }
        
        let mut vec: TinyVec<TestStruct> = TinyVec::new();
        
        let item1 = TestStruct {
            id: 1,
            name: "first".to_string(),
            active: true,
        };
        
        let item2 = TestStruct {
            id: 2,
            name: "second".to_string(),
            active: false,
        };
        
        vec.push(item1.clone());
        vec.push(item2.clone());
        
        assert_eq!(vec.len(), 2);
        assert_eq!(vec[0], item1);
        assert_eq!(vec[1], item2);
        
        // Test modification
        vec[0].active = false;
        assert_eq!(vec[0].active, false);
        assert_eq!(vec[0].id, 1);
    }

    #[test]
    fn test_large_stack_fill() {
        let mut vec: TinyVec<u64> = TinyVec::new();
        
        // Fill exactly to stack capacity
        for i in 0..2048 {
            vec.push(i as u64);
        }
        
        assert_eq!(vec.len(), 2048);
        assert_eq!(vec.capacity(), 2048);
        assert_eq!(vec[0], 0);
        assert_eq!(vec[2047], 2047);
        
        // Verify we're still on stack
        assert!(!vec.using_heap);
    }

    #[test]
    fn test_heap_growth() {
        let mut vec: TinyVec<u32> = TinyVec::new();
        
        // Fill beyond stack capacity
        for i in 0..3000 {
            vec.push(i);
        }
        
        assert_eq!(vec.len(), 3000);
        assert!(vec.capacity() >= 3000);
        assert!(vec.using_heap);
        
        // Verify elements are correct
        assert_eq!(vec[0], 0);
        assert_eq!(vec[2047], 2047);
        assert_eq!(vec[2048], 2048);
        assert_eq!(vec[2999], 2999);
    }

    #[test]
    fn test_edge_cases() {
        let mut vec = TinyVec::new();
        
        // Test empty operations
        assert_eq!(vec.pop(), None);
        assert_eq!(vec.get(0), None);
        assert_eq!(vec.get_mut(0), None);
        
        // Test single element
        vec.push(42);
        assert_eq!(vec.len(), 1);
        assert_eq!(vec[0], 42);
        
        // Test clear by popping all
        let popped = vec.pop().unwrap();
        assert_eq!(popped, 42);
        assert_eq!(vec.len(), 0);
        assert!(vec.is_empty());
    }

    #[test]
    fn test_clone() {
        let mut vec = TinyVec::new();
        vec.push(1);
        vec.push(2);
        vec.push(3);
        
        let cloned = vec.clone();
        assert_eq!(vec.len(), cloned.len());
        assert_eq!(vec[0], cloned[0]);
        assert_eq!(vec[1], cloned[1]);
        assert_eq!(vec[2], cloned[2]);
    }

    #[test]
    fn test_partial_eq() {
        let mut vec1 = TinyVec::new();
        vec1.push(1);
        vec1.push(2);
        
        let mut vec2 = TinyVec::new();
        vec2.push(1);
        vec2.push(2);
        
        assert_eq!(vec1, vec2);
        
        vec2[1] = 3;
        assert_ne!(vec1, vec2);
    }
}


