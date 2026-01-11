use std::io::{self, Write};

// Modern Agri-Tech color palette (ANSI escape codes)
pub mod colors {
    pub const RESET: &str = "\x1b[0m";
    pub const DIM: &str = "\x1b[2;37m";        // Punctuation: < > = / (dim white)
    pub const GREEN: &str = "\x1b[32m";        // Element names (fresh/growth)
    pub const CYAN: &str = "\x1b[36m";         // Attribute names (tech accent)
    pub const YELLOW: &str = "\x1b[33m";       // Attribute values (harvest gold)
    pub const WHITE: &str = "\x1b[97m";        // Text content (clean)
}

pub fn escape_xml(s: &str) -> String {
    s.replace('&', "&amp;")
     .replace('<', "&lt;")
     .replace('>', "&gt;")
     .replace('"', "&quot;")
}

pub fn write_tag_open(out: &mut impl Write, name: &str, use_color: bool) -> io::Result<()> {
    if use_color {
        write!(out, "{}<{}{}{}{}>{}",
            colors::DIM, colors::RESET, colors::GREEN, name, colors::DIM, colors::RESET)
    } else {
        write!(out, "<{}>", name)
    }
}

pub fn write_tag_close(out: &mut impl Write, name: &str, indent: usize, use_color: bool) -> io::Result<()> {
    let indent_str = "  ".repeat(indent);
    if use_color {
        writeln!(out, "{}{}<{}/{}{}{}>{}",
            indent_str, colors::DIM, colors::RESET, colors::GREEN, name, colors::DIM, colors::RESET)
    } else {
        writeln!(out, "{}</{}>", indent_str, name)
    }
}

pub fn write_file_tag_open(out: &mut impl Write, path: &str, use_color: bool) -> io::Result<()> {
    if use_color {
        writeln!(out, "  {}<{}{}File{} {}path{}={}{}\"{}\"{}>{}",
            colors::DIM, colors::RESET, colors::GREEN, colors::RESET,
            colors::CYAN, colors::DIM, colors::RESET, colors::YELLOW, escape_xml(path),
            colors::DIM, colors::RESET)
    } else {
        writeln!(out, "  <File path=\"{}\">", escape_xml(path))
    }
}

pub fn write_element_open_with_attrs(out: &mut impl Write, name: &str, attrs: &[(&str, &str)], use_color: bool) -> io::Result<()> {
    if use_color {
        write!(out, "{}<{}{}{}", colors::DIM, colors::RESET, colors::GREEN, escape_xml(name))?;
        for (attr_name, attr_value) in attrs {
            write!(out, "{} {}{}{}={}{}\"{}\"",
                colors::RESET, colors::CYAN, attr_name, colors::DIM, colors::RESET, colors::YELLOW, attr_value)?;
        }
        write!(out, "{}>{}",  colors::DIM, colors::RESET)?;
    } else {
        write!(out, "<{}", escape_xml(name))?;
        for (attr_name, attr_value) in attrs {
            write!(out, " {}=\"{}\"", attr_name, attr_value)?;
        }
        write!(out, ">")?;
    }
    Ok(())
}

pub fn write_element_with_attrs_and_text(out: &mut impl Write, name: &str, attrs: &[(&str, &str)], text: Option<&str>, use_color: bool) -> io::Result<()> {
    let escaped_name = escape_xml(name);
    if use_color {
        write!(out, "{}<{}{}{}", colors::DIM, colors::RESET, colors::GREEN, escaped_name)?;
        for (attr_name, attr_value) in attrs {
            write!(out, "{} {}{}{}={}{}\"{}\"",
                colors::RESET, colors::CYAN, attr_name, colors::DIM, colors::RESET, colors::YELLOW, attr_value)?;
        }
        write!(out, "{}>{}",  colors::DIM, colors::RESET)?;
        if let Some(t) = text {
            write!(out, "{}{}{}", colors::WHITE, escape_xml(t), colors::RESET)?;
        }
        write!(out, "{}<{}/{}{}{}>{}",
            colors::DIM, colors::RESET, colors::GREEN, escaped_name, colors::DIM, colors::RESET)?;
    } else {
        write!(out, "<{}", escaped_name)?;
        for (attr_name, attr_value) in attrs {
            write!(out, " {}=\"{}\"", attr_name, attr_value)?;
        }
        write!(out, ">")?;
        if let Some(t) = text {
            write!(out, "{}", escape_xml(t))?;
        }
        write!(out, "</{}>", escaped_name)?;
    }
    Ok(())
}

// New functions for semantic output

/// Write an empty element like <public/> for modifiers
pub fn write_empty_element(out: &mut impl Write, name: &str, indent: usize, use_color: bool) -> io::Result<()> {
    let indent_str = "  ".repeat(indent);
    if use_color {
        writeln!(out, "{}{}<{}{}{}/{}>{}",
            indent_str, colors::DIM, colors::RESET, colors::GREEN, name, colors::DIM, colors::RESET)
    } else {
        writeln!(out, "{}<{}/>", indent_str, name)
    }
}

/// Write element with compact location: start="line:col" end="line:col"
#[allow(dead_code)]
pub fn write_element_open_compact(out: &mut impl Write, name: &str, start: &str, end: &str, indent: usize, use_color: bool) -> io::Result<()> {
    let indent_str = "  ".repeat(indent);
    if use_color {
        write!(out, "{}{}<{}{}{}", indent_str, colors::DIM, colors::RESET, colors::GREEN, escape_xml(name))?;
        write!(out, "{} {}start{}={}{}\"{}\"",
            colors::RESET, colors::CYAN, colors::DIM, colors::RESET, colors::YELLOW, start)?;
        write!(out, "{} {}end{}={}{}\"{}\"",
            colors::RESET, colors::CYAN, colors::DIM, colors::RESET, colors::YELLOW, end)?;
        writeln!(out, "{}>{}",  colors::DIM, colors::RESET)?;
    } else {
        writeln!(out, "{}<{} start=\"{}\" end=\"{}\">", indent_str, escape_xml(name), start, end)?;
    }
    Ok(())
}

/// Write leaf element with compact location and text content
pub fn write_element_compact_with_text(out: &mut impl Write, name: &str, start: &str, end: &str, text: Option<&str>, indent: usize, use_color: bool) -> io::Result<()> {
    let indent_str = "  ".repeat(indent);
    let escaped_name = escape_xml(name);
    if use_color {
        write!(out, "{}{}<{}{}{}", indent_str, colors::DIM, colors::RESET, colors::GREEN, escaped_name)?;
        write!(out, "{} {}start{}={}{}\"{}\"",
            colors::RESET, colors::CYAN, colors::DIM, colors::RESET, colors::YELLOW, start)?;
        write!(out, "{} {}end{}={}{}\"{}\"",
            colors::RESET, colors::CYAN, colors::DIM, colors::RESET, colors::YELLOW, end)?;
        write!(out, "{}>{}",  colors::DIM, colors::RESET)?;
        if let Some(t) = text {
            write!(out, "{}{}{}", colors::WHITE, escape_xml(t), colors::RESET)?;
        }
        writeln!(out, "{}<{}/{}{}{}>{}",
            colors::DIM, colors::RESET, colors::GREEN, escaped_name, colors::DIM, colors::RESET)?;
    } else {
        write!(out, "{}<{} start=\"{}\" end=\"{}\">", indent_str, escaped_name, start, end)?;
        if let Some(t) = text {
            write!(out, "{}", escape_xml(t))?;
        }
        writeln!(out, "</{}>", escaped_name)?;
    }
    Ok(())
}

/// Write element with compact location and an extra attribute (e.g., fullName)
pub fn write_element_open_compact_with_attr(
    out: &mut impl Write,
    name: &str,
    start: &str,
    end: &str,
    extra_attr: Option<(&str, &str)>,
    indent: usize,
    use_color: bool
) -> io::Result<()> {
    let indent_str = "  ".repeat(indent);
    if use_color {
        write!(out, "{}{}<{}{}{}", indent_str, colors::DIM, colors::RESET, colors::GREEN, escape_xml(name))?;
        write!(out, "{} {}start{}={}{}\"{}\"",
            colors::RESET, colors::CYAN, colors::DIM, colors::RESET, colors::YELLOW, start)?;
        write!(out, "{} {}end{}={}{}\"{}\"",
            colors::RESET, colors::CYAN, colors::DIM, colors::RESET, colors::YELLOW, end)?;
        if let Some((attr_name, attr_value)) = extra_attr {
            write!(out, "{} {}{}{}={}{}\"{}\"",
                colors::RESET, colors::CYAN, attr_name, colors::DIM, colors::RESET, colors::YELLOW, escape_xml(attr_value))?;
        }
        writeln!(out, "{}>{}",  colors::DIM, colors::RESET)?;
    } else {
        write!(out, "{}<{} start=\"{}\" end=\"{}\"", indent_str, escape_xml(name), start, end)?;
        if let Some((attr_name, attr_value)) = extra_attr {
            write!(out, " {}=\"{}\"", attr_name, escape_xml(attr_value))?;
        }
        writeln!(out, ">")?;
    }
    Ok(())
}

/// Write closing tag with proper indentation
pub fn write_close_tag(out: &mut impl Write, name: &str, indent: usize, use_color: bool) -> io::Result<()> {
    let indent_str = "  ".repeat(indent);
    if use_color {
        writeln!(out, "{}{}<{}/{}{}{}>{}",
            indent_str, colors::DIM, colors::RESET, colors::GREEN, name, colors::DIM, colors::RESET)
    } else {
        writeln!(out, "{}</{}>", indent_str, name)
    }
}
