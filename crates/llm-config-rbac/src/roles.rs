//! Role definitions and assignments

use crate::permissions::{Action, Permission, Resource};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// Predefined roles in the system
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    /// System administrator with full access
    Admin,
    /// Can create, read, update configurations and secrets
    Editor,
    /// Can only read configurations (not secrets)
    Viewer,
    /// Can read audit logs and configurations
    Auditor,
    /// Custom role with specific permissions
    Custom(String),
}

impl Role {
    /// Get the permissions for this role
    pub fn permissions(&self) -> HashSet<Permission> {
        match self {
            Role::Admin => {
                // Admin has full access to everything
                let mut perms = HashSet::new();
                for resource in [
                    Resource::Config,
                    Resource::Secret,
                    Resource::History,
                    Resource::AuditLog,
                    Resource::System,
                    Resource::Users,
                    Resource::Roles,
                ] {
                    for action in [
                        Action::Read,
                        Action::Create,
                        Action::Update,
                        Action::Delete,
                        Action::List,
                        Action::Rollback,
                        Action::Export,
                        Action::Import,
                    ] {
                        perms.insert(Permission::new(resource.clone(), action));
                    }
                }
                perms
            }
            Role::Editor => {
                // Editor can manage configs and secrets but not system settings
                let mut perms = HashSet::new();
                for resource in [Resource::Config, Resource::Secret, Resource::History] {
                    for action in [
                        Action::Read,
                        Action::Create,
                        Action::Update,
                        Action::Delete,
                        Action::List,
                        Action::Rollback,
                    ] {
                        perms.insert(Permission::new(resource.clone(), action));
                    }
                }
                // Can read audit logs
                perms.insert(Permission::new(Resource::AuditLog, Action::Read));
                perms.insert(Permission::new(Resource::AuditLog, Action::List));
                perms
            }
            Role::Viewer => {
                // Viewer can only read configs (not secrets)
                let mut perms = HashSet::new();
                perms.insert(Permission::new(Resource::Config, Action::Read));
                perms.insert(Permission::new(Resource::Config, Action::List));
                perms.insert(Permission::new(Resource::History, Action::Read));
                perms.insert(Permission::new(Resource::History, Action::List));
                perms
            }
            Role::Auditor => {
                // Auditor can read everything but cannot modify
                let mut perms = HashSet::new();
                for resource in [
                    Resource::Config,
                    Resource::History,
                    Resource::AuditLog,
                    Resource::System,
                ] {
                    perms.insert(Permission::new(resource.clone(), Action::Read));
                    perms.insert(Permission::new(resource.clone(), Action::List));
                }
                // Can export for compliance reports
                perms.insert(Permission::new(Resource::AuditLog, Action::Export));
                perms
            }
            Role::Custom(_) => {
                // Custom roles need to be configured separately
                HashSet::new()
            }
        }
    }

    /// Check if this role has the given permission
    pub fn has_permission(&self, permission: &Permission) -> bool {
        self.permissions().contains(permission)
    }

    /// Check if this role can perform the action on the resource
    pub fn can(&self, resource: &Resource, action: &Action) -> bool {
        let permission = Permission::new(resource.clone(), action.clone());
        self.has_permission(&permission)
    }
}

/// Assignment of a role to a user
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoleAssignment {
    pub user: String,
    pub role: Role,
    /// Optional scope restriction (e.g., specific namespace)
    pub scope: Option<String>,
}

impl RoleAssignment {
    /// Create a new role assignment
    pub fn new(user: impl Into<String>, role: Role) -> Self {
        Self {
            user: user.into(),
            role,
            scope: None,
        }
    }

    /// Create a role assignment with a specific scope
    pub fn with_scope(user: impl Into<String>, role: Role, scope: impl Into<String>) -> Self {
        Self {
            user: user.into(),
            role,
            scope: Some(scope.into()),
        }
    }

    /// Check if this assignment applies to the given scope
    pub fn applies_to_scope(&self, scope: Option<&str>) -> bool {
        match (&self.scope, scope) {
            (None, _) => true,                // Global scope applies to everything
            (Some(s1), Some(s2)) => s1 == s2, // Exact scope match
            (Some(_), None) => false,         // Scoped assignment doesn't apply globally
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_admin_permissions() {
        let role = Role::Admin;
        assert!(role.can(&Resource::Config, &Action::Read));
        assert!(role.can(&Resource::Secret, &Action::Create));
        assert!(role.can(&Resource::System, &Action::Update));
        assert!(role.can(&Resource::Users, &Action::Delete));
    }

    #[test]
    fn test_editor_permissions() {
        let role = Role::Editor;
        assert!(role.can(&Resource::Config, &Action::Read));
        assert!(role.can(&Resource::Config, &Action::Update));
        assert!(role.can(&Resource::Secret, &Action::Create));
        assert!(role.can(&Resource::AuditLog, &Action::Read));

        // Cannot modify system settings
        assert!(!role.can(&Resource::System, &Action::Update));
        assert!(!role.can(&Resource::Users, &Action::Create));
    }

    #[test]
    fn test_viewer_permissions() {
        let role = Role::Viewer;
        assert!(role.can(&Resource::Config, &Action::Read));
        assert!(role.can(&Resource::History, &Action::Read));

        // Cannot modify anything
        assert!(!role.can(&Resource::Config, &Action::Update));
        assert!(!role.can(&Resource::Config, &Action::Delete));

        // Cannot access secrets
        assert!(!role.can(&Resource::Secret, &Action::Read));
    }

    #[test]
    fn test_auditor_permissions() {
        let role = Role::Auditor;
        assert!(role.can(&Resource::AuditLog, &Action::Read));
        assert!(role.can(&Resource::AuditLog, &Action::Export));
        assert!(role.can(&Resource::Config, &Action::Read));

        // Cannot modify anything
        assert!(!role.can(&Resource::Config, &Action::Update));
        assert!(!role.can(&Resource::AuditLog, &Action::Delete));
    }

    #[test]
    fn test_role_assignment() {
        let assignment = RoleAssignment::new("alice", Role::Editor);
        assert_eq!(assignment.user, "alice");
        assert_eq!(assignment.role, Role::Editor);
        assert!(assignment.scope.is_none());
    }

    #[test]
    fn test_scoped_assignment() {
        let assignment = RoleAssignment::with_scope("bob", Role::Editor, "namespace1");
        assert_eq!(assignment.user, "bob");
        assert!(assignment.applies_to_scope(Some("namespace1")));
        assert!(!assignment.applies_to_scope(Some("namespace2")));
        assert!(!assignment.applies_to_scope(None));
    }

    #[test]
    fn test_global_assignment() {
        let assignment = RoleAssignment::new("admin", Role::Admin);
        assert!(assignment.applies_to_scope(Some("namespace1")));
        assert!(assignment.applies_to_scope(Some("namespace2")));
        assert!(assignment.applies_to_scope(None));
    }
}
