/**
 * XPath query execution using fontoxpath
 */

import { evaluateXPathToNodes, evaluateXPath } from 'fontoxpath';

export interface Match {
  xml: string;
  value: string;
  start?: string;
  end?: string;
}

export type OutputFormat = 'xml' | 'lines' | 'source' | 'value' | 'gcc' | 'json' | 'count';

export const OUTPUT_FORMATS: { value: OutputFormat; label: string; description: string }[] = [
  { value: 'xml', label: 'XML', description: 'XML fragments of matched nodes' },
  { value: 'lines', label: 'Lines', description: 'Full source lines containing matches' },
  { value: 'source', label: 'Source', description: 'Exact matched source text' },
  { value: 'value', label: 'Value', description: 'Text content of matched nodes' },
  { value: 'gcc', label: 'GCC', description: 'file:line:col format for IDEs' },
  { value: 'json', label: 'JSON', description: 'JSON array with match details' },
  { value: 'count', label: 'Count', description: 'Number of matches only' },
];

/**
 * Find location attributes on element, its descendants, or ancestors
 * Some elements (field wrappers) don't have locations, but their children or parents do
 */
function findLocation(element: Element): { start?: string; end?: string } {
  // First try the element itself
  let start = element.getAttribute('start');
  let end = element.getAttribute('end');

  if (start && end) {
    return { start, end };
  }

  // If not found, look for first and last descendants with locations
  const allElements = element.getElementsByTagName('*');
  let firstStart: string | null = null;
  let lastEnd: string | null = null;

  for (let i = 0; i < allElements.length; i++) {
    const child = allElements[i];
    const childStart = child.getAttribute('start');
    const childEnd = child.getAttribute('end');

    if (childStart && !firstStart) {
      firstStart = childStart;
    }
    if (childEnd) {
      lastEnd = childEnd;
    }
  }

  if (firstStart && lastEnd) {
    return { start: firstStart, end: lastEnd };
  }

  // Still not found - walk up to parent elements
  let parent = element.parentElement;
  while (parent) {
    const parentStart = parent.getAttribute('start');
    const parentEnd = parent.getAttribute('end');
    if (parentStart && parentEnd) {
      // Found a parent with location - but this will highlight too much
      // For now, return it as a fallback
      return { start: parentStart, end: parentEnd };
    }
    parent = parent.parentElement;
  }

  return {
    start: start || firstStart || undefined,
    end: end || lastEnd || undefined,
  };
}

/**
 * Find location for a text node by looking at its parent element
 */
function findLocationForTextNode(textNode: Text): { start?: string; end?: string } {
  const parent = textNode.parentElement;
  if (parent) {
    return findLocation(parent);
  }
  return {};
}

/**
 * Execute an XPath query on XML and return matching nodes
 */
export function queryXml(xmlString: string, xpath: string): Match[] {
  // Parse XML string to DOM
  const parser = new DOMParser();
  const doc = parser.parseFromString(xmlString, 'text/xml');

  // Check for parse errors
  const parseError = doc.querySelector('parsererror');
  if (parseError) {
    throw new Error(`XML parse error: ${parseError.textContent}`);
  }

  // Try to evaluate as nodes first
  let nodes: Node[];
  try {
    nodes = evaluateXPathToNodes(xpath, doc);
  } catch {
    // If it fails (e.g., scalar result), try generic evaluation
    nodes = [];
  }

  // If no nodes returned, try evaluating as a scalar (string, number, boolean)
  if (nodes.length === 0) {
    try {
      const result = evaluateXPath(xpath, doc);
      // Handle scalar results
      if (result !== null && result !== undefined) {
        if (typeof result === 'string' || typeof result === 'number' || typeof result === 'boolean') {
          return [{
            xml: String(result),
            value: String(result),
            start: undefined,
            end: undefined,
          }];
        }
        // If it's an array (sequence), convert each item
        if (Array.isArray(result)) {
          return result.map((item) => ({
            xml: String(item),
            value: String(item),
            start: undefined,
            end: undefined,
          }));
        }
      }
    } catch {
      // If that also fails, return empty
      return [];
    }
  }

  // Filter out whitespace-only text nodes (common when using text())
  const filteredNodes = nodes.filter((node) => {
    if (node.nodeType === Node.TEXT_NODE) {
      const text = node.textContent || '';
      // Keep only text nodes with non-whitespace content
      return text.trim().length > 0;
    }
    return true;
  });

  // Convert results to matches
  const serializer = new XMLSerializer();

  return filteredNodes.map((node) => {
    // Handle different node types
    if (node.nodeType === Node.TEXT_NODE) {
      // Text node - get location from parent, value is just the text
      const textNode = node as Text;
      const location = findLocationForTextNode(textNode);
      const textContent = textNode.textContent || '';

      return {
        xml: textContent, // Text nodes don't have XML structure
        value: textContent,
        start: location.start,
        end: location.end,
      };
    } else if (node.nodeType === Node.ELEMENT_NODE) {
      // Element node - original handling
      const element = node as Element;
      const location = findLocation(element);

      return {
        xml: serializer.serializeToString(element),
        value: element.textContent || '',
        start: location.start,
        end: location.end,
      };
    } else if (node.nodeType === Node.ATTRIBUTE_NODE) {
      // Attribute node - return the attribute value
      const attr = node as Attr;
      const parent = attr.ownerElement;
      const location = parent ? findLocation(parent) : {};

      return {
        xml: `${attr.name}="${attr.value}"`,
        value: attr.value,
        start: location.start,
        end: location.end,
      };
    } else {
      // Other node types (comment, etc.) - fallback
      return {
        xml: node.textContent || '',
        value: node.textContent || '',
        start: undefined,
        end: undefined,
      };
    }
  });
}
