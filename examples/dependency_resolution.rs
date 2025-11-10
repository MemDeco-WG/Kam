/// Example demonstrating the dependency resolution features of Kam
/// 
/// This example shows how to:
/// - Define dependency groups
/// - Use includes to create hierarchical dependencies
/// - Resolve dependencies with cycle detection
/// - Handle errors in dependency resolution

use std::collections::BTreeMap;
use kam::dependency_resolver::DependencyResolver;
use kam::types::kam_toml::Dependency;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Kam Dependency Resolution Examples ===\n");

    // Example 1: Simple dependency resolution
    example_simple_resolution()?;

    // Example 2: Using includes
    example_with_includes()?;

    // Example 3: Nested includes
    example_nested_includes()?;

    // Example 4: Cycle detection
    example_cycle_detection();

    // Example 5: Missing group detection
    example_missing_group();

    println!("\n=== All Examples Completed ===");
    Ok(())
}

fn create_dependency(id: &str, version: Option<&str>) -> Dependency {
    Dependency {
        id: id.to_string(),
        version: version.map(|v| v.to_string()),
        source: None,
    }
}

fn example_simple_resolution() -> Result<(), Box<dyn std::error::Error>> {
    println!("Example 1: Simple Dependency Resolution");
    println!("----------------------------------------");

    let mut groups = BTreeMap::new();
    groups.insert(
        "normal".to_string(),
        vec![
            create_dependency("base-lib", Some("1.0.0")),
            create_dependency("utils", Some("2.0.0")),
        ],
    );

    let resolver = DependencyResolver::from_groups(groups);
    let resolved = resolver.resolve()?;

    if let Some(normal_group) = resolved.get("normal") {
        println!("Resolved 'normal' group:");
        for dep in &normal_group.dependencies {
            println!("  - {} ({})", dep.id, dep.version.as_ref().unwrap_or(&"*".to_string()));
        }
    }

    println!();
    Ok(())
}

fn example_with_includes() -> Result<(), Box<dyn std::error::Error>> {
    println!("Example 2: Using Includes");
    println!("-------------------------");

    let mut groups = BTreeMap::new();
    groups.insert(
        "normal".to_string(),
        vec![
            create_dependency("core-lib", Some("1.0.0")),
            create_dependency("common-utils", Some("2.0.0")),
        ],
    );
    groups.insert(
        "dev".to_string(),
        vec![
            create_dependency("include:normal", None),  // Include all normal dependencies
            create_dependency("test-framework", Some("3.0.0")),
            create_dependency("debugger", Some("1.5.0")),
        ],
    );

    let resolver = DependencyResolver::from_groups(groups);
    let resolved = resolver.resolve()?;

    if let Some(dev_group) = resolved.get("dev") {
        println!("Resolved 'dev' group (includes 'normal'):");
        for dep in &dev_group.dependencies {
            println!("  - {} ({})", dep.id, dep.version.as_ref().unwrap_or(&"*".to_string()));
        }
    }

    println!();
    Ok(())
}

fn example_nested_includes() -> Result<(), Box<dyn std::error::Error>> {
    println!("Example 3: Nested Includes");
    println!("--------------------------");

    let mut groups = BTreeMap::new();
    groups.insert(
        "base".to_string(),
        vec![create_dependency("fundamental-lib", Some("1.0.0"))],
    );
    groups.insert(
        "runtime".to_string(),
        vec![
            create_dependency("include:base", None),
            create_dependency("runtime-lib", Some("2.0.0")),
        ],
    );
    groups.insert(
        "dev".to_string(),
        vec![
            create_dependency("include:runtime", None),
            create_dependency("test-lib", Some("3.0.0")),
        ],
    );

    let resolver = DependencyResolver::from_groups(groups);
    let resolved = resolver.resolve()?;

    if let Some(dev_group) = resolved.get("dev") {
        println!("Resolved 'dev' group (includes 'runtime' which includes 'base'):");
        for dep in &dev_group.dependencies {
            println!("  - {} ({})", dep.id, dep.version.as_ref().unwrap_or(&"*".to_string()));
        }
    }

    println!();
    Ok(())
}

fn example_cycle_detection() {
    println!("Example 4: Cycle Detection");
    println!("--------------------------");

    let mut groups = BTreeMap::new();
    groups.insert(
        "group-a".to_string(),
        vec![create_dependency("include:group-b", None)],
    );
    groups.insert(
        "group-b".to_string(),
        vec![create_dependency("include:group-a", None)],
    );

    let resolver = DependencyResolver::from_groups(groups);
    match resolver.resolve() {
        Ok(_) => println!("ERROR: Should have detected a cycle!"),
        Err(e) => {
            println!("✓ Correctly detected cycle:");
            println!("  Error: {}", e);
        }
    }

    println!();
}

fn example_missing_group() {
    println!("Example 5: Missing Group Detection");
    println!("-----------------------------------");

    let mut groups = BTreeMap::new();
    groups.insert(
        "normal".to_string(),
        vec![create_dependency("include:nonexistent", None)],
    );

    let resolver = DependencyResolver::from_groups(groups);
    match resolver.resolve() {
        Ok(_) => println!("ERROR: Should have detected missing group!"),
        Err(e) => {
            println!("✓ Correctly detected missing group:");
            println!("  Error: {}", e);
        }
    }

    println!();
}
