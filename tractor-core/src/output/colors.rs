//! ANSI color support for terminal output

use regex::Regex;

/// ANSI color codes
pub mod ansi {
    pub const RESET: &str = "\x1b[0m";
    pub const DIM: &str = "\x1b[2m";
    pub const BOLD: &str = "\x1b[1m";
    pub const BLUE: &str = "\x1b[34m";
    pub const CYAN: &str = "\x1b[36m";
    pub const YELLOW: &str = "\x1b[33m";
    pub const GREEN: &str = "\x1b[32m";
    pub const GRAY: &str = "\x1b[90m";
    pub const MAGENTA: &str = "\x1b[35m";
    pub const BLACK: &str = "\x1b[30m";
    pub const BG_YELLOW: &str = "\x1b[43m";
    pub const BG_GREEN: &str = "\x1b[42m";
}

/// Determine if color should be used based on mode and environment
pub fn should_use_color(mode: &str) -> bool {
    match mode {
        "always" => true,
        "never" => false,
        _ => {
            // Auto mode: check if stdout is a tty and NO_COLOR is not set
            atty::is(atty::Stream::Stdout) && std::env::var("NO_COLOR").is_err()
        }
    }
}

/// Colorize XML output for better readability
pub fn colorize_xml(xml: &str) -> String {
    if xml.is_empty() {
        return xml.to_string();
    }

    let mut result = xml.to_string();

    // XML declaration: <?xml ... ?>
    let decl_re = Regex::new(r"(<\?)(xml)([^?]*)(\?>)").unwrap();
    result = decl_re.replace_all(&result, |caps: &regex::Captures| {
        format!(
            "{}{}{}{}{}",
            ansi::DIM,
            &caps[1],
            &caps[2],
            &caps[3],
            ansi::RESET
        )
    }).to_string();

    // Comments: <!-- ... -->
    let comment_re = Regex::new(r"(<!--)(.*?)(-->)").unwrap();
    result = comment_re.replace_all(&result, |caps: &regex::Captures| {
        format!(
            "{}{}{}{}{}",
            ansi::DIM,
            &caps[1],
            &caps[2],
            &caps[3],
            ansi::RESET
        )
    }).to_string();

    // Attributes: name="value" (simplified regex without backreference)
    let attr_re = Regex::new(r#"(\s)([a-zA-Z_][\w\-]*)(\s*=\s*")([^"]*)""#).unwrap();
    result = attr_re.replace_all(&result, |caps: &regex::Captures| {
        format!(
            "{}{}{}{}{}{}\"{}{}{}{}\"{}",
            &caps[1],
            ansi::CYAN,
            &caps[2],
            ansi::RESET,
            ansi::DIM,
            &caps[3],
            ansi::RESET,
            ansi::YELLOW,
            &caps[4],
            ansi::RESET,
            ansi::DIM,
        ) + ansi::RESET
    }).to_string();

    // Opening tags: <elementName
    let tag_re = Regex::new(r"(<)(/?)([\w][\w\-\.]*)").unwrap();
    result = tag_re.replace_all(&result, |caps: &regex::Captures| {
        format!(
            "{}{}{}{}{}{}",
            ansi::DIM,
            &caps[1],
            &caps[2],
            ansi::RESET,
            ansi::BLUE,
            &caps[3],
        ) + ansi::RESET
    }).to_string();

    // Closing brackets: > and />
    let bracket_re = Regex::new(r"(\s*/?>)").unwrap();
    result = bracket_re.replace_all(&result, |caps: &regex::Captures| {
        format!("{}{}{}", ansi::DIM, &caps[1], ansi::RESET)
    }).to_string();

    result
}

/// Colorize XML with highlighted matches
pub fn colorize_xml_with_highlights(xml: &str, match_positions: &[(String, u32, u32)], use_color: bool) -> String {
    if xml.is_empty() {
        return xml.to_string();
    }

    let lines: Vec<&str> = xml.split('\n').collect();
    let mut result = Vec::new();

    // Create regex to find elements with location attributes
    let elem_re = Regex::new(r#"<(\w+)(\s+[^>]*?start="(\d+):(\d+)"[^>]*)(>|/>)"#).unwrap();

    for line in lines {
        let mut processed_line = line.to_string();
        let mut line_has_match = false;

        // Check for matches in this line
        processed_line = elem_re.replace_all(&processed_line, |caps: &regex::Captures| {
            let element_name = &caps[1];
            let attrs = &caps[2];
            let close_bracket = &caps[5];
            let start_line: u32 = caps[3].parse().unwrap_or(0);
            let start_col: u32 = caps[4].parse().unwrap_or(0);

            let is_match = match_positions.iter().any(|(name, l, c)| {
                name == element_name && *l == start_line && *c == start_col
            });

            if is_match {
                line_has_match = true;
                if use_color {
                    format!(
                        "{}{}{}<{}{}{}{}",
                        ansi::BG_YELLOW,
                        ansi::BLACK,
                        ansi::BOLD,
                        element_name,
                        attrs,
                        close_bracket,
                        ansi::RESET
                    )
                } else {
                    format!("<{}{}{}  <<<MATCH", element_name, attrs, close_bracket)
                }
            } else {
                caps[0].to_string()
            }
        }).to_string();

        // Add line marker for matched lines
        if line_has_match {
            let marker = ">> ";
            if use_color {
                processed_line = format!(
                    "{}{}{}{}{}",
                    ansi::BG_YELLOW,
                    ansi::BLACK,
                    marker,
                    ansi::RESET,
                    &processed_line[marker.len().min(processed_line.len())..]
                );
            } else {
                processed_line = format!("{}{}", marker, &processed_line[marker.len().min(processed_line.len())..]);
            }
        }

        result.push(processed_line);
    }

    // Apply syntax coloring to non-highlighted lines
    let combined = result.join("\n");
    if use_color {
        let final_lines: Vec<String> = combined
            .split('\n')
            .map(|line| {
                if !line.contains(ansi::BG_YELLOW) {
                    colorize_xml(line)
                } else {
                    line.to_string()
                }
            })
            .collect();
        final_lines.join("\n")
    } else {
        combined
    }
}
