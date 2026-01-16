/**
 * Parse XML string into a tree structure for the query builder
 */

export interface XmlNode {
  id: string;
  name: string;          // Element name (e.g., "class", "method")
  attributes: Record<string, string>;
  textContent: string | null;  // For leaf nodes with text
  children: XmlNode[];
  depth: number;
}

let nodeCounter = 0;

/**
 * Parse XML string into XmlNode tree
 */
export function parseXmlToTree(xmlString: string): XmlNode | null {
  nodeCounter = 0;

  const parser = new DOMParser();
  const doc = parser.parseFromString(xmlString, 'text/xml');

  // Check for parse errors
  const parseError = doc.querySelector('parsererror');
  if (parseError) {
    console.error('XML parse error:', parseError.textContent);
    return null;
  }

  // Find the root content element (skip Files/File wrapper)
  const filesEl = doc.documentElement;
  if (filesEl.tagName === 'Files') {
    const fileEl = filesEl.querySelector('File');
    if (fileEl && fileEl.firstElementChild) {
      return convertElement(fileEl.firstElementChild, 0);
    }
  }

  // Fallback: use document element
  return convertElement(doc.documentElement, 0);
}

function convertElement(element: Element, depth: number): XmlNode {
  const id = `node-${nodeCounter++}`;

  // Get attributes
  const attributes: Record<string, string> = {};
  for (const attr of element.attributes) {
    attributes[attr.name] = attr.value;
  }

  // Get children (only elements, skip text nodes except for leaf content)
  const children: XmlNode[] = [];
  let textContent: string | null = null;

  const childElements = Array.from(element.children);

  if (childElements.length === 0) {
    // Leaf node - get text content
    const text = element.textContent?.trim();
    if (text) {
      textContent = text;
    }
  } else {
    // Has child elements
    for (const child of childElements) {
      children.push(convertElement(child, depth + 1));
    }
  }

  return {
    id,
    name: element.tagName,
    attributes,
    textContent,
    children,
    depth,
  };
}

/**
 * Get unique ID for an XML node
 */
export function getXmlNodeId(node: XmlNode): string {
  return node.id;
}
