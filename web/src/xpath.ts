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

    return {
      xml: serializer.serializeToString(element),
      start: element.getAttribute('start') ?? undefined,
      end: element.getAttribute('end') ?? undefined,
    };
  });
}
