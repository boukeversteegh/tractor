/**
 * XPath query execution using fontoxpath
 */

import { evaluateXPath, evaluateXPathToNodes } from 'fontoxpath';

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

/**
 * Execute an XPath query and return a string result
 */
export function queryXmlToString(xmlString: string, xpath: string): string {
  const parser = new DOMParser();
  const doc = parser.parseFromString(xmlString, 'text/xml');

  const parseError = doc.querySelector('parsererror');
  if (parseError) {
    throw new Error(`XML parse error: ${parseError.textContent}`);
  }

  return evaluateXPath(xpath, doc, null, null, { returnType: evaluateXPath.STRING_TYPE }) as string;
}

/**
 * Count matches for an XPath query
 */
export function countMatches(xmlString: string, xpath: string): number {
  const parser = new DOMParser();
  const doc = parser.parseFromString(xmlString, 'text/xml');

  const parseError = doc.querySelector('parsererror');
  if (parseError) {
    throw new Error(`XML parse error: ${parseError.textContent}`);
  }

  return evaluateXPath(`count(${xpath})`, doc, null, null, { returnType: evaluateXPath.NUMBER_TYPE }) as number;
}
