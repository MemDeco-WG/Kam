use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use crate::types::kam_toml::sections::dependency::{Dependency, DependencySection};

/// A flattened dependency group with all includes resolved
#[derive(Debug, Clone)]
pub struct FlatDependencyGroup {
    pub dependencies: Vec<Dependency>,
}

impl Default for FlatDependencyGroup {
    fn default() -> Self {
        Self {
            dependencies: Vec::new(),
        }
    }
}

/// Container for all flattened dependency groups
#[derive(Debug, Default, Clone)]
pub struct FlatDependencyGroups {
    groups: BTreeMap<String, FlatDependencyGroup>,
}

impl FlatDependencyGroups {
    /// Create a new empty FlatDependencyGroups
    pub fn new() -> Self {
        Self::default()
    }

    /// Get a flattened dependency group by name
    pub fn get(&self, name: &str) -> Option<&FlatDependencyGroup> {
        self.groups.get(name)
    }

    /// Insert a flattened dependency group
    pub fn insert(&mut self, name: String, group: FlatDependencyGroup) {
        self.groups.insert(name, group);
    }

    /// Iterate over all groups
    pub fn iter(&self) -> impl Iterator<Item = (&String, &FlatDependencyGroup)> {
        self.groups.iter()
    }

    /// Get all group names
    pub fn group_names(&self) -> Vec<&String> {
        self.groups.keys().collect()
    }
}

/// Error types for dependency resolution
#[derive(Debug, Clone)]
pub enum DependencyResolutionError {
    /// A dependency group was not found
    GroupNotFound {
        group: String,
        referenced_by: String,
    },
    /// A cycle was detected in group includes
    CycleDetected {
        cycle: Vec<String>,
    },
    /// Invalid include syntax
    InvalidInclude {
        group: String,
        include_spec: String,
    },
}

impl fmt::Display for DependencyResolutionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::GroupNotFound { group, referenced_by } => {
                write!(
                    f,
                    "Dependency group '{}' not found (referenced by '{}')",
                    group, referenced_by
                )
            }
            Self::CycleDetected { cycle } => {
                write!(f, "Cycle detected in dependency groups: ")?;
                if let Some((first, rest)) = cycle.split_first() {
                    write!(f, "`{}`", first)?;
                    for group in rest {
                        write!(f, " -> `{}`", group)?;
                    }
                    write!(f, " -> `{}`", first)?;
                }
                Ok(())
            }
            Self::InvalidInclude { group, include_spec } => {
                write!(
                    f,
                    "Invalid include syntax in group '{}': '{}'",
                    group, include_spec
                )
            }
        }
    }
}

impl std::error::Error for DependencyResolutionError {}

/// Dependency resolver that handles group includes and cycle detection
pub struct DependencyResolver {
    groups: BTreeMap<String, Vec<DependencyEntry>>,
}

/// An entry in a dependency group - either a concrete dependency or an include
#[derive(Debug, Clone)]
enum DependencyEntry {
    Dependency(Dependency),
    Include(String),
}

impl DependencyResolver {
    /// Create a new resolver from a DependencySection
    pub fn new(dependency_section: &DependencySection) -> Self {
        let mut groups = BTreeMap::new();

        // Add normal dependencies
        if let Some(normal) = &dependency_section.normal {
            let entries = normal
                .iter()
                .map(|dep| {
                    // Check if this is an include (id starts with "include:")
                    if dep.id.starts_with("include:") {
                        DependencyEntry::Include(dep.id.strip_prefix("include:").unwrap().to_string())
                    } else {
                        DependencyEntry::Dependency(dep.clone())
                    }
                })
                .collect();
            groups.insert("normal".to_string(), entries);
        }

        // Add dev dependencies
        if let Some(dev) = &dependency_section.dev {
            let entries = dev
                .iter()
                .map(|dep| {
                    // Check if this is an include (id starts with "include:")
                    if dep.id.starts_with("include:") {
                        DependencyEntry::Include(dep.id.strip_prefix("include:").unwrap().to_string())
                    } else {
                        DependencyEntry::Dependency(dep.clone())
                    }
                })
                .collect();
            groups.insert("dev".to_string(), entries);
        }

        Self { groups }
    }

    /// Create a resolver from a map of named dependency groups
    pub fn from_groups(groups_map: BTreeMap<String, Vec<Dependency>>) -> Self {
        let mut groups = BTreeMap::new();

        for (name, deps) in groups_map {
            let entries = deps
                .iter()
                .map(|dep| {
                    // Check if this is an include (id starts with "include:")
                    if dep.id.starts_with("include:") {
                        DependencyEntry::Include(dep.id.strip_prefix("include:").unwrap().to_string())
                    } else {
                        DependencyEntry::Dependency(dep.clone())
                    }
                })
                .collect();
            groups.insert(name, entries);
        }

        Self { groups }
    }

    /// Resolve all dependency groups, handling includes and detecting cycles
    pub fn resolve(&self) -> Result<FlatDependencyGroups, DependencyResolutionError> {
        let mut resolved = BTreeMap::new();

        for group_name in self.groups.keys() {
            let mut visited = BTreeSet::new();
            let deps = self.resolve_group(group_name, &mut visited)?;
            resolved.insert(
                group_name.clone(),
                FlatDependencyGroup { dependencies: deps },
            );
        }

        Ok(FlatDependencyGroups { groups: resolved })
    }

    /// Recursively resolve a single group
    fn resolve_group(
        &self,
        group_name: &str,
        visited: &mut BTreeSet<String>,
    ) -> Result<Vec<Dependency>, DependencyResolutionError> {
        // Check for cycles
        if visited.contains(group_name) {
            let mut cycle: Vec<String> = visited.iter().cloned().collect();
            cycle.push(group_name.to_string());
            return Err(DependencyResolutionError::CycleDetected { cycle });
        }

        // Get the group
        let entries = match self.groups.get(group_name) {
            Some(entries) => entries,
            None => {
                let referenced_by = visited
                    .iter()
                    .last()
                    .map(|s| s.as_str())
                    .unwrap_or("root");
                return Err(DependencyResolutionError::GroupNotFound {
                    group: group_name.to_string(),
                    referenced_by: referenced_by.to_string(),
                });
            }
        };

        visited.insert(group_name.to_string());

        let mut dependencies = Vec::new();

        for entry in entries {
            match entry {
                DependencyEntry::Dependency(dep) => {
                    dependencies.push(dep.clone());
                }
                DependencyEntry::Include(included_group) => {
                    // Recursively resolve the included group
                    let included_deps = self.resolve_group(included_group, visited)?;
                    dependencies.extend(included_deps);
                }
            }
        }

        visited.remove(group_name);

        Ok(dependencies)
    }

    /// Validate that all includes reference existing groups
    pub fn validate(&self) -> Result<(), DependencyResolutionError> {
        for (group_name, entries) in &self.groups {
            for entry in entries {
                if let DependencyEntry::Include(included_group) = entry {
                    if !self.groups.contains_key(included_group) {
                        return Err(DependencyResolutionError::GroupNotFound {
                            group: included_group.clone(),
                            referenced_by: group_name.clone(),
                        });
                    }
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_dependency(id: &str, version: Option<&str>) -> Dependency {
        Dependency {
            id: id.to_string(),
            version: version.map(|v| v.to_string()),
            source: None,
        }
    }

    #[test]
    fn test_simple_resolution() {
        let mut groups = BTreeMap::new();
        groups.insert(
            "normal".to_string(),
            vec![
                create_dependency("dep1", Some("1.0.0")),
                create_dependency("dep2", Some("2.0.0")),
            ],
        );

        let resolver = DependencyResolver::from_groups(groups);
        let result = resolver.resolve().unwrap();

        let normal_group = result.get("normal").unwrap();
        assert_eq!(normal_group.dependencies.len(), 2);
        assert_eq!(normal_group.dependencies[0].id, "dep1");
        assert_eq!(normal_group.dependencies[1].id, "dep2");
    }

    #[test]
    fn test_include_resolution() {
        let mut groups = BTreeMap::new();
        groups.insert(
            "base".to_string(),
            vec![
                create_dependency("base-dep1", Some("1.0.0")),
                create_dependency("base-dep2", Some("2.0.0")),
            ],
        );
        groups.insert(
            "extended".to_string(),
            vec![
                create_dependency("include:base", None),
                create_dependency("extended-dep", Some("3.0.0")),
            ],
        );

        let resolver = DependencyResolver::from_groups(groups);
        let result = resolver.resolve().unwrap();

        let extended_group = result.get("extended").unwrap();
        assert_eq!(extended_group.dependencies.len(), 3);
        assert_eq!(extended_group.dependencies[0].id, "base-dep1");
        assert_eq!(extended_group.dependencies[1].id, "base-dep2");
        assert_eq!(extended_group.dependencies[2].id, "extended-dep");
    }

    #[test]
    fn test_cycle_detection() {
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
        let result = resolver.resolve();

        assert!(result.is_err());
        match result.unwrap_err() {
            DependencyResolutionError::CycleDetected { cycle } => {
                assert!(cycle.len() >= 2);
            }
            _ => panic!("Expected CycleDetected error"),
        }
    }

    #[test]
    fn test_missing_group() {
        let mut groups = BTreeMap::new();
        groups.insert(
            "normal".to_string(),
            vec![create_dependency("include:nonexistent", None)],
        );

        let resolver = DependencyResolver::from_groups(groups);
        let result = resolver.resolve();

        assert!(result.is_err());
        match result.unwrap_err() {
            DependencyResolutionError::GroupNotFound { group, .. } => {
                assert_eq!(group, "nonexistent");
            }
            _ => panic!("Expected GroupNotFound error"),
        }
    }

    #[test]
    fn test_validate() {
        let mut groups = BTreeMap::new();
        groups.insert(
            "normal".to_string(),
            vec![create_dependency("include:nonexistent", None)],
        );

        let resolver = DependencyResolver::from_groups(groups);
        let result = resolver.validate();

        assert!(result.is_err());
    }

    #[test]
    fn test_nested_includes() {
        let mut groups = BTreeMap::new();
        groups.insert(
            "base".to_string(),
            vec![create_dependency("base-dep", Some("1.0.0"))],
        );
        groups.insert(
            "middle".to_string(),
            vec![
                create_dependency("include:base", None),
                create_dependency("middle-dep", Some("2.0.0")),
            ],
        );
        groups.insert(
            "top".to_string(),
            vec![
                create_dependency("include:middle", None),
                create_dependency("top-dep", Some("3.0.0")),
            ],
        );

        let resolver = DependencyResolver::from_groups(groups);
        let result = resolver.resolve().unwrap();

        let top_group = result.get("top").unwrap();
        assert_eq!(top_group.dependencies.len(), 3);
        assert_eq!(top_group.dependencies[0].id, "base-dep");
        assert_eq!(top_group.dependencies[1].id, "middle-dep");
        assert_eq!(top_group.dependencies[2].id, "top-dep");
    }
}
