using System.Xml.XPath;

namespace CodeXTractor;

public record Match(
    string File,
    int Line,
    int Column,
    int EndLine,
    int EndColumn,
    string Value,
    string[] SourceLines,
    XPathNavigator? Navigator
);
