using System.Xml.XPath;

namespace Tractor.XPath;

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
