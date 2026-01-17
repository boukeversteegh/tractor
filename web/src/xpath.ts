/**
 * XPath query execution using fontoxpath
 */

import { evaluateXPathToNodes } from 'fontoxpath';

export interface Match {
  xml: string;
  start?: string;
  end?: string;
}

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

  // Execute XPath query
  const nodes = evaluateXPathToNodes(xpath, doc);

  // Convert results to matches
  return nodes.map((node) => {
    const element = node as Element;
    const serializer = new XMLSerializer();
    const location = findLocation(element);

    return {
      xml: serializer.serializeToString(element),
      start: location.start,
      end: location.end,
    };
  });
}
