use tera::Tera;

/// Initializes the Tera template engine by loading all templates from the specified directory.
pub fn init_renderer() -> Tera {
    let mut tera = Tera::new("templates/**/*").unwrap();
    tera.add_template_file("src/templates/base.html", Some("base.html"))
        .unwrap();
    tera
}

/// Renders a template file with the given variables and returns the resulting string.
pub fn render_template(
    tera: &mut Tera,
    path: &str,
    variables: &std::collections::HashMap<String, String>,
) -> Result<String, String> {
    tera.add_template_file(path, Some(path))
        .map_err(|e| e.to_string())?;

    // Prepare the context with some data
    let mut context = tera::Context::new();
    for (key, value) in variables {
        context.insert(key, value);
    }

    // Render the template with the given context
    tera.render(path, &context).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn make_base_vars(title: &str) -> HashMap<String, String> {
        let mut vars = HashMap::new();
        vars.insert("title".to_string(), title.to_string());
        vars.insert(
            "static_path".to_string(),
            "/static/css/styles.css?v=abc123".to_string(),
        );
        vars
    }

    #[test]
    fn render_template_index_substitutes_title() {
        let mut tera = init_renderer();
        let vars = make_base_vars("Robia Labs Ltd");
        let output = render_template(&mut tera, "src/templates/index.html", &vars).unwrap();
        assert!(output.contains("Robia Labs Ltd"));
    }

    #[test]
    fn render_template_index_substitutes_static_path() {
        let mut tera = init_renderer();
        let vars = make_base_vars("Robia Labs Ltd");
        let output = render_template(&mut tera, "src/templates/index.html", &vars).unwrap();
        // Tera HTML-escapes '/' as '&#x2F;', so check the unescaped path components
        assert!(output.contains("styles.css?v=abc123"));
    }

    #[test]
    fn render_template_500_contains_error_heading() {
        let mut tera = init_renderer();
        let vars = make_base_vars("500 Internal Server Error");
        let output = render_template(&mut tera, "src/templates/500.html", &vars).unwrap();
        assert!(output.contains("500 Internal Server Error"));
        assert!(output.contains("Oops! Something went wrong."));
    }

    #[test]
    fn render_template_500_substitutes_static_path() {
        let mut tera = init_renderer();
        let vars = make_base_vars("500 Internal Server Error");
        let output = render_template(&mut tera, "src/templates/500.html", &vars).unwrap();
        // Tera HTML-escapes '/' as '&#x2F;', so check the unescaped path components
        assert!(output.contains("styles.css?v=abc123"));
    }

    #[test]
    fn render_template_login_contains_sign_in_button() {
        let mut tera = init_renderer();
        let vars = make_base_vars("Login");
        let output = render_template(&mut tera, "src/templates/login.html", &vars).unwrap();
        assert!(output.contains("Sign In"));
    }

    #[test]
    fn render_template_login_contains_username_and_password_inputs() {
        let mut tera = init_renderer();
        let vars = make_base_vars("Login");
        let output = render_template(&mut tera, "src/templates/login.html", &vars).unwrap();
        assert!(output.contains(r#"id="username""#));
        assert!(output.contains(r#"id="password""#));
    }

    #[test]
    fn render_template_login_substitutes_static_path() {
        let mut tera = init_renderer();
        let vars = make_base_vars("Login");
        let output = render_template(&mut tera, "src/templates/login.html", &vars).unwrap();
        assert!(output.contains("styles.css?v=abc123"));
    }

    #[test]
    fn render_template_returns_err_for_missing_template() {
        let mut tera = init_renderer();
        let vars = make_base_vars("Test");
        let result = render_template(&mut tera, "src/templates/nonexistent.html", &vars);
        assert!(result.is_err());
    }
}
