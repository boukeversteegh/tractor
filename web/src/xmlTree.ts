/**
 * Parse XML string into a tree structure for the query builder
 */

export interface Position {
  line: number;  // 1-indexed
  column: number;  // 1-indexed
}

export interface Location {
  start: Position;
  end: Position;
}

export interface XmlNode {
  id: string;
  name: string;          // Element name (e.g., "class", "method")
  attributes: Record<string, string>;
  textContent: string | null;  // For leaf nodes with text
  children: XmlNode[];
  depth: number;
  location: Location | null;  // Parsed from start/end attributes
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

/**
 * Parse location string "line:col" into Position
 */
function parsePosition(str: string): Position | null {
  const match = str.match(/^(\d+):(\d+)$/);
  if (!match) return null;
  return {
    line: parseInt(match[1], 10),
    column: parseInt(match[2], 10),
  };
}

/**
 * Parse start/end attributes into Location
 */
function parseLocation(attributes: Record<string, string>): Location | null {
  const startStr = attributes['start'];
  const endStr = attributes['end'];
  if (!startStr || !endStr) return null;

  const start = parsePosition(startStr);
  const end = parsePosition(endStr);
  if (!start || !end) return null;

  return { start, end };
}

function convertElement(element: Element, depth: number): XmlNode {
  const id = `node-${nodeCounter++}`;

  // Get attributes
  const attributes: Record<string, string> = {};
  for (const attr of element.attributes) {
    attributes[attr.name] = attr.value;
  }

  // Parse location from attributes
  const location = parseLocation(attributes);

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
    location,
  };
}

/**
 * Get unique ID for an XML node
 */
export function getXmlNodeId(node: XmlNode): string {
  return node.id;
}

/**
 * Check if a position is within a location range
 */
function isPositionInLocation(pos: Position, loc: Location): boolean {
  // Before start?
  if (pos.line < loc.start.line) return false;
  if (pos.line === loc.start.line && pos.column < loc.start.column) return false;

  // After end?
  if (pos.line > loc.end.line) return false;
  if (pos.line === loc.end.line && pos.column > loc.end.column) return false;

  return true;
}

/**
 * Find the deepest node containing the given position
 */
export function findDeepestNodeAtPosition(
  tree: XmlNode,
  position: Position
): XmlNode | null {
  // If this node has a location, check if position is within it
  if (tree.location) {
    if (!isPositionInLocation(position, tree.location)) {
      return null;
    }
  }
  // If node has no location (field wrapper), still check children

  // Check children for deeper match
  for (const child of tree.children) {
    const deeper = findDeepestNodeAtPosition(child, position);
    if (deeper) return deeper;
  }

  // This node contains the position but no children do
  // Only return this node if it has a location (skip field wrappers)
  return tree.location ? tree : null;
}

/**
 * Get the path (list of ancestor IDs) from root to the given node
 */
export function getPathToNode(
  tree: XmlNode,
  targetId: string,
  currentPath: string[] = []
): string[] | null {
  const newPath = [...currentPath, tree.id];

  if (tree.id === targetId) {
    return newPath;
  }

  for (const child of tree.children) {
    const path = getPathToNode(child, targetId, newPath);
    if (path) return path;
  }

  return null;
}

/**
 * Convert cursor offset (character position) to line:column
 */
export function offsetToPosition(source: string, offset: number): Position {
  let line = 1;
  let column = 1;

  for (let i = 0; i < offset && i < source.length; i++) {
    if (source[i] === '\n') {
      line++;
      column = 1;
    } else {
      column++;
    }
  }

  return { line, column };
}

/**
 * Convert line:column position to character offset
 */
export function positionToOffset(source: string, position: Position): number {
  let currentLine = 1;
  let currentColumn = 1;

  for (let i = 0; i < source.length; i++) {
    if (currentLine === position.line && currentColumn === position.column) {
      return i;
    }

    if (source[i] === '\n') {
      currentLine++;
      currentColumn = 1;
    } else {
      currentColumn++;
    }
  }

  // If we reached the end and we're at the target line, return end position
  if (currentLine === position.line) {
    return source.length;
  }

  return source.length;
}

/**
 * Parse position string "line:col" into Position
 */
export function parsePositionString(str: string): Position | null {
  const match = str.match(/^(\d+):(\d+)$/);
  if (!match) return null;
  return {
    line: parseInt(match[1], 10),
    column: parseInt(match[2], 10),
  };
}

/**
 * Get the path of node names from root to this node.
 * Returns an array like ['class', 'body', 'method', 'name'].
 * This is used as the selection key - selecting any node with this path
 * selects ALL nodes matching that pattern.
 */
export function getNodeNamePath(node: XmlNode, ancestors: string[] = []): string[] {
  return [...ancestors, node.name];
}

/**
 * Convert a name path array to a string key for selection state.
 * e.g., ['class', 'body', 'method'] -> 'class/body/method'
 */
export function pathToKey(path: string[]): string {
  return path.join('/');
}

/**
 * Convert a path key string back to an array.
 * e.g., 'class/body/method' -> ['class', 'body', 'method']
 */
export function keyToPath(key: string): string[] {
  return key.split('/');
}

/**
 * Compute the path key (e.g., "class/body/method") for a target node
 * by walking the tree to find it and building the ancestor name path.
 */
export function computePathKeyForNode(
  tree: XmlNode,
  target: XmlNode,
  currentNames: string[] = [],
): string | null {
  const names = [...currentNames, tree.name];

  if (tree.id === target.id) {
    return names.join('/');
  }

  for (const child of tree.children) {
    const result = computePathKeyForNode(child, target, names);
    if (result) return result;
  }

  return null;
}

/**
 * Find all nodes in the tree that match a given name path.
 * Since paths don't include indexes, this returns all instances.
 */
export function findNodesByPath(
  tree: XmlNode,
  targetPath: string[],
  currentPath: string[] = []
): XmlNode[] {
  const nodePath = [...currentPath, tree.name];
  const results: XmlNode[] = [];

  // Check if this node matches the target path
  if (nodePath.length === targetPath.length &&
      nodePath.every((name, i) => name === targetPath[i])) {
    results.push(tree);
  }

  // Continue searching in children (path might match deeper nodes too)
  for (const child of tree.children) {
    results.push(...findNodesByPath(child, targetPath, nodePath));
  }

  return results;
}
