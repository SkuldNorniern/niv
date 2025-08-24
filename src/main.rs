use niv_rope::RBTree;

fn main() {
    println!("Red-Black Tree Implementation Demo");
    println!("================================\n");

    // Create a new Red-Black Tree
    let mut tree = RBTree::new();
    println!("Created new Red-Black Tree");

    // Insert some keys
    let keys = vec![10, 20, 5, 15, 25, 3, 7, 12, 18, 30];
    
    println!("\nInserting keys: {:?}", keys);
    for &key in &keys {
        match tree.insert(key) {
            Ok(()) => println!("✓ Inserted key: {}", key),
            Err(e) => println!("✗ Failed to insert key {}: {}", key, e),
        }
    }

    // Display the tree structure
    println!("\nTree structure after insertions:");
    tree.debug_print();

    // Verify Red-Black properties
    println!("\nValidating Red-Black Tree properties...");
    if tree.is_valid() {
        println!("✓ Tree maintains all Red-Black properties");
    } else {
        println!("✗ Tree violates Red-Black properties");
    }

    // Search for various keys
    println!("\nSearching for keys:");
    let search_keys = vec![10, 15, 25, 3, 100, 0];
    
    for &key in &search_keys {
        match tree.search(key) {
            Some(node_id) => println!("✓ Found key {} at node ID: {}", key, node_id),
            None => println!("✗ Key {} not found in tree", key),
        }
    }

    // Test with a different set of keys
    println!("\n{}", "=".repeat(50));
    println!("Testing with different key set...");
    
    let mut tree2 = RBTree::new();
    let keys2 = vec![50, 30, 70, 20, 40, 60, 80, 10, 25, 35, 45, 55, 65, 75, 85];
    
    println!("Inserting keys: {:?}", keys2);
    for &key in &keys2 {
        tree2.insert(key).expect("Insertion should succeed");
    }
    
    println!("\nSecond tree structure:");
    tree2.debug_print();
    
    println!("\nSecond tree validation: {}", if tree2.is_valid() { "✓ Valid" } else { "✗ Invalid" });
    
    // Performance demonstration
    println!("\n{}", "=".repeat(50));
    println!("Performance demonstration with larger tree...");
    
    let mut large_tree = RBTree::new();
    let large_keys: Vec<u64> = (1..=100000000).collect();
    
    println!("Inserting 100000000 sequential keys...");
    for &key in &large_keys {
        large_tree.insert(key).expect("Insertion should succeed");
    }
    
    println!("Large tree validation: {}", if large_tree.is_valid() { "✓ Valid" } else { "✗ Invalid" });
    
    // Search performance test
    let search_targets = vec![1, 50, 100, 25, 75, 99999999];
    println!("\nSearching in large tree:");
    for &target in &search_targets {
        let start = std::time::Instant::now();
        let result = large_tree.search(target);
        let duration = start.elapsed();
        
        match result {
            Some(_) => println!("✓ Found {} in {:?}", target, duration),
            None => println!("✗ {} not found (searched in {:?})", target, duration),
        }
    }
    
    println!("\nRed-Black Tree demo completed successfully!");
}
