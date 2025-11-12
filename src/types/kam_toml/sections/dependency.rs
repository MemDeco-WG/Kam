use crate::errors::KamError;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};

/// Version specification for dependencies
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(untagged)]
pub enum VersionSpec {
    /// Exact version code
    Exact(i64),
    /// Version range (e.g., "[1000,2000)")
    Range(String),
}

impl VersionSpec {
    pub fn as_display(&self) -> String {
        match self {
            VersionSpec::Exact(v) => v.to_string(),
            VersionSpec::Range(r) => r.clone(),
        }
    }
}

/// A dependency entry
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[allow(non_snake_case)]
pub struct Dependency {
    /// Module ID
    pub id: String,
    /// Version specification
    pub versionCode: Option<VersionSpec>,
    /// Optional source URL
    pub source: Option<String>,
}

/// Dependency section with kam and dev groups
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct DependencySection {
    /// Runtime dependencies
    pub kam: Option<Vec<Dependency>>,
    /// Development dependencies
    pub dev: Option<Vec<Dependency>>,
}

impl Default for DependencySection {
    fn default() -> Self {
        DependencySection {
            kam: Some(Vec::new()),
            dev: Some(Vec::new()),
        }
    }
}

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
    groups: std::collections::BTreeMap<String, FlatDependencyGroup>,
}

impl FlatDependencyGroups {
    /// Get a flattened dependency group by name
    pub fn get(&self, name: &str) -> Option<&FlatDependencyGroup> {
        self.groups.get(name)
    }
}

impl DependencySection {
    /// Resolve dependencies into flattened groups, supporting include syntax with recursion and cycle detection
    pub fn resolve(&self) -> crate::errors::Result<FlatDependencyGroups> {
        use std::collections::{BTreeMap, HashSet};

        let mut groups = BTreeMap::new();
        let mut visited = HashSet::new();

        // Resolve each predefined group
        self.resolve_group("kam", &mut groups, &mut visited)?;
        self.resolve_group("dev", &mut groups, &mut visited)?;

        Ok(FlatDependencyGroups { groups })
    }

    /// Recursively resolve a dependency group, handling includes
    fn resolve_group(
        &self,
        group_name: &str,
        resolved_groups: &mut BTreeMap<String, FlatDependencyGroup>,
        visited: &mut HashSet<String>,
    ) -> crate::errors::Result<()> {
        if !visited.insert(group_name.to_string()) {
            return Err(KamError::DependencyResolutionFailed(format!(
                "Circular dependency detected involving group '{}'",
                group_name
            )));
        }

        let empty = Vec::new();
        let deps = match group_name {
            "kam" => self.kam.as_ref().unwrap_or(&empty),
            "dev" => self.dev.as_ref().unwrap_or(&empty),
            _ => {
                return Err(KamError::DependencyResolutionFailed(format!(
                    "Unknown dependency group '{}'",
                    group_name
                )));
            }
        };

        let mut flattened = Vec::new();

        for dep in deps {
            if let Some(include_group) = dep.id.strip_prefix("include:") {
                // Recursively resolve the included group
                self.resolve_group(include_group, resolved_groups, visited)?;
                // Add the dependencies from the included group
                if let Some(included) = resolved_groups.get(include_group) {
                    flattened.extend(included.dependencies.clone());
                }
            } else {
                flattened.push(dep.clone());
            }
        }

        resolved_groups.insert(
            group_name.to_string(),
            FlatDependencyGroup {
                dependencies: flattened,
            },
        );

        visited.remove(group_name);

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_simple() {
        let dep_section = DependencySection {
            kam: Some(vec![Dependency {
                id: "lib1".to_string(),
                versionCode: Some(VersionSpec::Exact(100i64)),
                source: None,
            }]),
            dev: Some(vec![Dependency {
                id: "lib2".to_string(),
                versionCode: Some(VersionSpec::Exact(200i64)),
                source: None,
            }]),
        };

        let result = dep_section.resolve().unwrap();
        assert_eq!(result.get("kam").unwrap().dependencies.len(), 1);
        assert_eq!(result.get("kam").unwrap().dependencies[0].id, "lib1");
        assert_eq!(result.get("dev").unwrap().dependencies.len(), 1);
        assert_eq!(result.get("dev").unwrap().dependencies[0].id, "lib2");
    }

    #[test]
    fn test_resolve_with_include() {
        let dep_section = DependencySection {
            kam: Some(vec![
                Dependency {
                    id: "lib1".to_string(),
                    versionCode: Some(VersionSpec::Exact(100i64)),
                    source: None,
                },
                Dependency {
                    id: "include:dev".to_string(),
                    versionCode: None,
                    source: None,
                },
            ]),
            dev: Some(vec![Dependency {
                id: "lib2".to_string(),
                versionCode: Some(VersionSpec::Exact(200)),
                source: None,
            }]),
        };

        let result = dep_section.resolve().unwrap();
        assert_eq!(result.get("kam").unwrap().dependencies.len(), 2);
        assert_eq!(result.get("kam").unwrap().dependencies[0].id, "lib1");
        assert_eq!(result.get("kam").unwrap().dependencies[1].id, "lib2");
        assert_eq!(result.get("dev").unwrap().dependencies.len(), 1);
        assert_eq!(result.get("dev").unwrap().dependencies[0].id, "lib2");
    }

    #[test]
    fn test_resolve_circular_dependency() {
        let dep_section = DependencySection {
            kam: Some(vec![Dependency {
                id: "include:dev".to_string(),
                versionCode: None,
                source: None,
            }]),
            dev: Some(vec![Dependency {
                id: "include:kam".to_string(),
                versionCode: None,
                source: None,
            }]),
        };

        let result = dep_section.resolve();
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Circular dependency")
        );
    }

    #[test]
    fn test_resolve_unknown_group() {
        let dep_section = DependencySection {
            kam: Some(vec![Dependency {
                id: "include:unknown".to_string(),
                versionCode: None,
                source: None,
            }]),
            dev: None,
        };

        let result = dep_section.resolve();
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Unknown dependency group")
        );
    }
}
