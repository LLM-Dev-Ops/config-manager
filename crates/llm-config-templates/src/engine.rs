//! Template engine for managing and rendering templates

use crate::{parser, Result, Template, TemplateError};
use std::collections::HashMap;

/// Template engine for rendering templates
pub struct TemplateEngine {
    templates: HashMap<String, Template>,
}

impl TemplateEngine {
    /// Create a new template engine
    pub fn new() -> Self {
        Self {
            templates: HashMap::new(),
        }
    }

    /// Register a template
    pub fn register(&mut self, template: Template) {
        self.templates.insert(template.name.clone(), template);
    }

    /// Get a template by name
    pub fn get(&self, name: &str) -> Option<&Template> {
        self.templates.get(name)
    }

    /// Remove a template
    pub fn remove(&mut self, name: &str) -> Option<Template> {
        self.templates.remove(name)
    }

    /// Render a template by name
    pub fn render_template(&self, name: &str, vars: &HashMap<String, String>) -> Result<String> {
        let template = self.templates.get(name).ok_or_else(|| {
            TemplateError::InvalidTemplate(format!("Template not found: {}", name))
        })?;

        template.render(vars)
    }

    /// Render a template string directly (without registration)
    pub fn render(&self, template_str: &str, vars: &HashMap<String, String>) -> Result<String> {
        let tokens = parser::parse(template_str)?;
        let mut result = String::new();

        for token in tokens {
            match token {
                parser::Token::Literal(text) => result.push_str(&text),
                parser::Token::Variable(var_name) => {
                    let value = vars
                        .get(&var_name)
                        .ok_or(TemplateError::VariableNotFound(var_name))?;
                    result.push_str(value);
                }
            }
        }

        Ok(result)
    }

    /// List all registered templates
    pub fn list_templates(&self) -> Vec<&str> {
        self.templates.keys().map(|s| s.as_str()).collect()
    }

    /// Get the number of registered templates
    pub fn count(&self) -> usize {
        self.templates.len()
    }
}

impl Default for TemplateEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_engine_creation() {
        let engine = TemplateEngine::new();
        assert_eq!(engine.count(), 0);
    }

    #[test]
    fn test_register_template() {
        let mut engine = TemplateEngine::new();
        let template = Template::new("test", "Hello, {{name}}!").unwrap();

        engine.register(template);
        assert_eq!(engine.count(), 1);
    }

    #[test]
    fn test_get_template() {
        let mut engine = TemplateEngine::new();
        let template = Template::new("test", "Hello, {{name}}!").unwrap();

        engine.register(template);

        let retrieved = engine.get("test");
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().name, "test");
    }

    #[test]
    fn test_remove_template() {
        let mut engine = TemplateEngine::new();
        let template = Template::new("test", "Hello, {{name}}!").unwrap();

        engine.register(template);
        assert_eq!(engine.count(), 1);

        let removed = engine.remove("test");
        assert!(removed.is_some());
        assert_eq!(engine.count(), 0);
    }

    #[test]
    fn test_render_template() {
        let mut engine = TemplateEngine::new();
        let template = Template::new("greeting", "Hello, {{name}}!").unwrap();
        engine.register(template);

        let mut vars = HashMap::new();
        vars.insert("name".to_string(), "Alice".to_string());

        let result = engine.render_template("greeting", &vars).unwrap();
        assert_eq!(result, "Hello, Alice!");
    }

    #[test]
    fn test_render_direct() {
        let engine = TemplateEngine::new();

        let mut vars = HashMap::new();
        vars.insert("name".to_string(), "Bob".to_string());
        vars.insert("age".to_string(), "30".to_string());

        let result = engine
            .render("Name: {{name}}, Age: {{age}}", &vars)
            .unwrap();
        assert_eq!(result, "Name: Bob, Age: 30");
    }

    #[test]
    fn test_list_templates() {
        let mut engine = TemplateEngine::new();
        engine.register(Template::new("t1", "{{var1}}").unwrap());
        engine.register(Template::new("t2", "{{var2}}").unwrap());
        engine.register(Template::new("t3", "{{var3}}").unwrap());

        let templates = engine.list_templates();
        assert_eq!(templates.len(), 3);
        assert!(templates.contains(&"t1"));
        assert!(templates.contains(&"t2"));
        assert!(templates.contains(&"t3"));
    }

    #[test]
    fn test_template_not_found() {
        let engine = TemplateEngine::new();
        let vars = HashMap::new();

        let result = engine.render_template("nonexistent", &vars);
        assert!(result.is_err());
    }

    #[test]
    fn test_variable_not_found() {
        let engine = TemplateEngine::new();
        let vars = HashMap::new();

        let result = engine.render("Hello, {{name}}!", &vars);
        assert!(result.is_err());
    }
}
