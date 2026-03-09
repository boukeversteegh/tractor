// ---------------------------------------------------------------------------
// XPath normalization
// ---------------------------------------------------------------------------

pub fn is_msys_environment() -> bool {
    std::env::var("MSYSTEM").is_ok()
}

pub fn normalize_xpath(xpath: &str) -> String {
    let xpath = fix_msys_xpath_mangling(xpath);

    if xpath.starts_with('/')
        || xpath.starts_with('(')
        || xpath.starts_with('$')
        || xpath.starts_with('"')
        || xpath.starts_with('\'')
        || xpath == "."
        || looks_like_xpath_expression(&xpath)
    {
        xpath
    } else {
        format!("//{}", xpath)
    }
}

pub fn looks_like_xpath_expression(xpath: &str) -> bool {
    let keywords = ["let ", "let$", "for ", "for$", "if ", "if(", "some ", "some$", "every ", "every$"];
    keywords.iter().any(|kw| xpath.starts_with(kw))
        || xpath.starts_with("not(")
        || xpath.starts_with("count(")
        || xpath.starts_with("string(")
        || xpath.starts_with("contains(")
        || xpath.starts_with("starts-with(")
        || xpath.chars().next().map_or(false, |c| c.is_ascii_digit())
}

pub fn fix_msys_xpath_mangling(xpath: &str) -> String {
    if !is_msys_environment() {
        return xpath.to_string();
    }

    if xpath.starts_with('/') && !xpath.starts_with("//") {
        let rest = &xpath[1..];
        if !rest.is_empty() && (rest.chars().next().unwrap().is_alphabetic() || rest.starts_with('*')) {
            return format!("/{}", xpath);
        }
    }

    xpath.to_string()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_auto_prefixes_bare_element_names() {
        assert_eq!(normalize_xpath("function"), "//function");
        assert_eq!(normalize_xpath("variable"), "//variable");
        assert_eq!(normalize_xpath("class"), "//class");
        assert_eq!(normalize_xpath("name"), "//name");
    }

    #[test]
    fn test_normalize_preserves_absolute_paths() {
        assert_eq!(normalize_xpath("//function"), "//function");
        assert_eq!(normalize_xpath("//class[name='Foo']"), "//class[name='Foo']");
        if !is_msys_environment() {
            assert_eq!(normalize_xpath("/root"), "/root");
        }
    }

    #[test]
    fn test_normalize_preserves_parenthesized_expressions() {
        assert_eq!(normalize_xpath("(//a | //b)"), "(//a | //b)");
    }

    #[test]
    fn test_normalize_preserves_dot() {
        assert_eq!(normalize_xpath("."), ".");
    }

    #[test]
    fn test_normalize_preserves_let_expressions() {
        assert_eq!(
            normalize_xpath("let $v := //function return $v/name"),
            "let $v := //function return $v/name"
        );
        assert_eq!(
            normalize_xpath("let$v := //x return $v"),
            "let$v := //x return $v"
        );
    }

    #[test]
    fn test_normalize_preserves_for_expressions() {
        assert_eq!(
            normalize_xpath("for $v in //name return string($v)"),
            "for $v in //name return string($v)"
        );
        assert_eq!(
            normalize_xpath("for$v in //name return $v"),
            "for$v in //name return $v"
        );
    }

    #[test]
    fn test_normalize_preserves_if_expressions() {
        assert_eq!(
            normalize_xpath("if (//x) then 1 else 0"),
            "if (//x) then 1 else 0"
        );
        assert_eq!(
            normalize_xpath("if(//x) then 1 else 0"),
            "if(//x) then 1 else 0"
        );
    }

    #[test]
    fn test_normalize_preserves_quantified_expressions() {
        assert_eq!(
            normalize_xpath("some $v in //x satisfies $v/name"),
            "some $v in //x satisfies $v/name"
        );
        assert_eq!(
            normalize_xpath("every $v in //x satisfies $v/name"),
            "every $v in //x satisfies $v/name"
        );
    }

    #[test]
    fn test_normalize_preserves_variable_references() {
        assert_eq!(normalize_xpath("$var"), "$var");
    }

    #[test]
    fn test_normalize_preserves_string_literals() {
        assert_eq!(normalize_xpath("\"hello\""), "\"hello\"");
        assert_eq!(normalize_xpath("'hello'"), "'hello'");
    }

    #[test]
    fn test_normalize_preserves_numeric_literals() {
        assert_eq!(normalize_xpath("42"), "42");
        assert_eq!(normalize_xpath("3.14"), "3.14");
    }

    #[test]
    fn test_normalize_preserves_function_calls() {
        assert_eq!(normalize_xpath("count(//item)"), "count(//item)");
        assert_eq!(normalize_xpath("not(//x)"), "not(//x)");
        assert_eq!(normalize_xpath("string(//x)"), "string(//x)");
        assert_eq!(normalize_xpath("contains(//x, 'y')"), "contains(//x, 'y')");
        assert_eq!(normalize_xpath("starts-with(//x, 'y')"), "starts-with(//x, 'y')");
    }
}
