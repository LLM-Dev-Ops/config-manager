//! Policy enforcement engine

use crate::{
    permissions::{Action, Resource},
    roles::RoleAssignment,
    RbacError, Result,
};
use std::collections::HashMap;

/// Policy enforcer that checks permissions based on role assignments
pub struct PolicyEnforcer {
    assignments: HashMap<String, Vec<RoleAssignment>>,
}

impl PolicyEnforcer {
    /// Create a new policy enforcer
    pub fn new() -> Self {
        Self {
            assignments: HashMap::new(),
        }
    }

    /// Assign a role to a user
    pub fn assign_role(&mut self, assignment: RoleAssignment) {
        self.assignments
            .entry(assignment.user.clone())
            .or_default()
            .push(assignment);
    }

    /// Remove a role from a user
    pub fn revoke_role(&mut self, user: &str, role_index: usize) -> Result<()> {
        let assignments = self
            .assignments
            .get_mut(user)
            .ok_or_else(|| RbacError::UserNotFound(user.to_string()))?;

        if role_index >= assignments.len() {
            return Err(RbacError::InvalidRole(format!(
                "Role index {} out of bounds",
                role_index
            )));
        }

        assignments.remove(role_index);

        // Remove user entry if no more assignments
        if assignments.is_empty() {
            self.assignments.remove(user);
        }

        Ok(())
    }

    /// Get all role assignments for a user
    pub fn get_user_roles(&self, user: &str) -> Vec<&RoleAssignment> {
        self.assignments
            .get(user)
            .map(|v| v.iter().collect())
            .unwrap_or_default()
    }

    /// Check if a user can perform an action on a resource
    pub fn check_permission(
        &self,
        user: &str,
        resource: &Resource,
        action: &Action,
        scope: Option<&str>,
    ) -> Result<()> {
        let assignments = self.get_user_roles(user);

        if assignments.is_empty() {
            return Err(RbacError::AccessDenied(format!(
                "User {} has no role assignments",
                user
            )));
        }

        // Check if any of the user's roles allows the action
        for assignment in assignments {
            if assignment.applies_to_scope(scope) && assignment.role.can(resource, action) {
                return Ok(());
            }
        }

        Err(RbacError::AccessDenied(format!(
            "User {} is not authorized to {} on {}",
            user, action, resource
        )))
    }

    /// Check if a user has permission (returns bool instead of Result)
    pub fn has_permission(
        &self,
        user: &str,
        resource: &Resource,
        action: &Action,
        scope: Option<&str>,
    ) -> bool {
        self.check_permission(user, resource, action, scope).is_ok()
    }

    /// List all users with role assignments
    pub fn list_users(&self) -> Vec<&str> {
        self.assignments.keys().map(|s| s.as_str()).collect()
    }

    /// Get total number of role assignments
    pub fn assignment_count(&self) -> usize {
        self.assignments.values().map(|v| v.len()).sum()
    }
}

impl Default for PolicyEnforcer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::roles::Role;

    #[test]
    fn test_enforcer_creation() {
        let enforcer = PolicyEnforcer::new();
        assert_eq!(enforcer.assignment_count(), 0);
    }

    #[test]
    fn test_assign_role() {
        let mut enforcer = PolicyEnforcer::new();
        let assignment = RoleAssignment::new("alice", Role::Editor);
        enforcer.assign_role(assignment);

        assert_eq!(enforcer.assignment_count(), 1);
        assert_eq!(enforcer.get_user_roles("alice").len(), 1);
    }

    #[test]
    fn test_multiple_roles() {
        let mut enforcer = PolicyEnforcer::new();
        enforcer.assign_role(RoleAssignment::new("alice", Role::Editor));
        enforcer.assign_role(RoleAssignment::with_scope(
            "alice",
            Role::Admin,
            "special-namespace",
        ));

        assert_eq!(enforcer.assignment_count(), 2);
        assert_eq!(enforcer.get_user_roles("alice").len(), 2);
    }

    #[test]
    fn test_check_permission_allowed() {
        let mut enforcer = PolicyEnforcer::new();
        enforcer.assign_role(RoleAssignment::new("alice", Role::Editor));

        assert!(enforcer
            .check_permission("alice", &Resource::Config, &Action::Read, None)
            .is_ok());
        assert!(enforcer
            .check_permission("alice", &Resource::Secret, &Action::Create, None)
            .is_ok());
    }

    #[test]
    fn test_check_permission_denied() {
        let mut enforcer = PolicyEnforcer::new();
        enforcer.assign_role(RoleAssignment::new("bob", Role::Viewer));

        // Viewer can read configs
        assert!(enforcer
            .check_permission("bob", &Resource::Config, &Action::Read, None)
            .is_ok());

        // But cannot update them
        assert!(enforcer
            .check_permission("bob", &Resource::Config, &Action::Update, None)
            .is_err());

        // And cannot access secrets
        assert!(enforcer
            .check_permission("bob", &Resource::Secret, &Action::Read, None)
            .is_err());
    }

    #[test]
    fn test_scoped_permissions() {
        let mut enforcer = PolicyEnforcer::new();

        // Alice is viewer globally
        enforcer.assign_role(RoleAssignment::new("alice", Role::Viewer));

        // But admin for "special" namespace
        enforcer.assign_role(RoleAssignment::with_scope("alice", Role::Admin, "special"));

        // Can read globally
        assert!(enforcer
            .check_permission("alice", &Resource::Config, &Action::Read, None)
            .is_ok());

        // Cannot update globally
        assert!(enforcer
            .check_permission("alice", &Resource::Config, &Action::Update, None)
            .is_err());

        // Can update in special namespace
        assert!(enforcer
            .check_permission("alice", &Resource::Config, &Action::Update, Some("special"))
            .is_ok());

        // Cannot update in other namespaces
        assert!(enforcer
            .check_permission("alice", &Resource::Config, &Action::Update, Some("other"))
            .is_err());
    }

    #[test]
    fn test_revoke_role() {
        let mut enforcer = PolicyEnforcer::new();
        enforcer.assign_role(RoleAssignment::new("alice", Role::Editor));

        assert_eq!(enforcer.assignment_count(), 1);

        enforcer.revoke_role("alice", 0).unwrap();

        assert_eq!(enforcer.assignment_count(), 0);
        assert_eq!(enforcer.get_user_roles("alice").len(), 0);
    }

    #[test]
    fn test_list_users() {
        let mut enforcer = PolicyEnforcer::new();
        enforcer.assign_role(RoleAssignment::new("alice", Role::Editor));
        enforcer.assign_role(RoleAssignment::new("bob", Role::Viewer));
        enforcer.assign_role(RoleAssignment::new("charlie", Role::Admin));

        let users = enforcer.list_users();
        assert_eq!(users.len(), 3);
        assert!(users.contains(&"alice"));
        assert!(users.contains(&"bob"));
        assert!(users.contains(&"charlie"));
    }

    #[test]
    fn test_has_permission() {
        let mut enforcer = PolicyEnforcer::new();
        enforcer.assign_role(RoleAssignment::new("alice", Role::Editor));

        assert!(enforcer.has_permission("alice", &Resource::Config, &Action::Read, None));
        assert!(!enforcer.has_permission("alice", &Resource::System, &Action::Update, None));
    }

    #[test]
    fn test_no_role_assignment() {
        let enforcer = PolicyEnforcer::new();

        assert!(enforcer
            .check_permission("unknown", &Resource::Config, &Action::Read, None)
            .is_err());
    }
}
