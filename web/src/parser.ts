/**
 * TreeSitter parser wrapper with AST serialization
 */

import { Parser, Language } from 'web-tree-sitter';

/** Serialized node format matching Rust's SerializedNode */
export interface SerializedNode {
  kind: string;
  isNamed: boolean;
  startRow: number;
  startCol: number;
  endRow: number;
  endCol: number;
  startByte: number;
  endByte: number;
  fieldName?: string;
  children: SerializedNode[];
}

// Import grammar wasm files from npm packages (Vite handles these)
// @ts-ignore
import treeSitterWasm from 'web-tree-sitter/web-tree-sitter.wasm?url';
// @ts-ignore
import rustWasm from 'tree-sitter-rust/tree-sitter-rust.wasm?url';
// @ts-ignore
import javascriptWasm from 'tree-sitter-javascript/tree-sitter-javascript.wasm?url';
// @ts-ignore
import typescriptWasm from 'tree-sitter-typescript/tree-sitter-typescript.wasm?url';
// @ts-ignore
import tsxWasm from 'tree-sitter-typescript/tree-sitter-tsx.wasm?url';
// @ts-ignore
import pythonWasm from 'tree-sitter-python/tree-sitter-python.wasm?url';
// @ts-ignore
import csharpWasm from 'tree-sitter-c-sharp/tree-sitter-c_sharp.wasm?url';
// @ts-ignore
import goWasm from 'tree-sitter-go/tree-sitter-go.wasm?url';
// @ts-ignore
import javaWasm from 'tree-sitter-java/tree-sitter-java.wasm?url';
// @ts-ignore
import cWasm from 'tree-sitter-c/tree-sitter-c.wasm?url';
// @ts-ignore
import cppWasm from 'tree-sitter-cpp/tree-sitter-cpp.wasm?url';
// @ts-ignore
import jsonWasm from 'tree-sitter-json/tree-sitter-json.wasm?url';
// @ts-ignore
import htmlWasm from 'tree-sitter-html/tree-sitter-html.wasm?url';
// @ts-ignore
import cssWasm from 'tree-sitter-css/tree-sitter-css.wasm?url';
// @ts-ignore
import bashWasm from 'tree-sitter-bash/tree-sitter-bash.wasm?url';

/** Language to grammar URL mapping */
const GRAMMAR_URLS: Record<string, string> = {
  typescript: typescriptWasm,
  javascript: javascriptWasm,
  tsx: tsxWasm,
  csharp: csharpWasm,
  rust: rustWasm,
  python: pythonWasm,
  go: goWasm,
  java: javaWasm,
  cpp: cppWasm,
  c: cWasm,
  json: jsonWasm,
  html: htmlWasm,
  css: cssWasm,
  bash: bashWasm,
};

let parser: Parser | null = null;
const loadedLanguages = new Map<string, Language>();

/**
 * Initialize the TreeSitter parser
 */
export async function initParser(): Promise<void> {
  await Parser.init({
    locateFile: () => treeSitterWasm,
  });
  parser = new Parser();
}

/**
 * Load a language grammar
 */
export async function loadLanguage(lang: string): Promise<Language> {
  if (loadedLanguages.has(lang)) {
    return loadedLanguages.get(lang)!;
  }

  const grammarUrl = GRAMMAR_URLS[lang];
  if (!grammarUrl) {
    throw new Error(`Unsupported language: ${lang}`);
  }

  const language = await Language.load(grammarUrl);
  loadedLanguages.set(lang, language);
  return language;
}

/**
 * Parse source code and return serialized AST
 */
export async function parseSource(source: string, lang: string): Promise<SerializedNode> {
  if (!parser) {
    await initParser();
  }

  const language = await loadLanguage(lang);
  parser!.setLanguage(language);

  const tree = parser!.parse(source);
  if (!tree) {
    throw new Error('Failed to parse source');
  }
  return serializeNode(tree.rootNode);
}

/**
 * Serialize a TreeSitter node to our JSON format
 */
function serializeNode(node: any, fieldName?: string): SerializedNode {
  const children: SerializedNode[] = [];

  // Use cursor to iterate children with field names
  const cursor = node.walk();
  if (cursor.gotoFirstChild()) {
    do {
      const child = cursor.currentNode;
      const childFieldName = cursor.currentFieldName ?? undefined;
      children.push(serializeNode(child, childFieldName));
    } while (cursor.gotoNextSibling());
  }

  return {
    kind: node.type,
    isNamed: node.isNamed,
    startRow: node.startPosition.row,
    startCol: node.startPosition.column,
    endRow: node.endPosition.row,
    endCol: node.endPosition.column,
    startByte: node.startIndex,
    endByte: node.endIndex,
    fieldName,
    children,
  };
}

/**
 * Get list of supported languages
 */
export function getSupportedLanguages(): string[] {
  return Object.keys(GRAMMAR_URLS);
}
