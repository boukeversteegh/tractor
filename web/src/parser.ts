/**
 * TreeSitter parser wrapper with AST serialization
 */

// web-tree-sitter types
type TreeSitterParser = {
  init(options?: { locateFile?: (path: string) => string }): Promise<void>;
  Language: {
    load(path: string): Promise<any>;
  };
  new(): any;
};

let TreeSitter: TreeSitterParser;

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

/** Language to grammar file mapping */
const GRAMMAR_FILES: Record<string, string> = {
  typescript: 'tree-sitter-typescript.wasm',
  javascript: 'tree-sitter-javascript.wasm',
  tsx: 'tree-sitter-tsx.wasm',
  csharp: 'tree-sitter-c_sharp.wasm',
  rust: 'tree-sitter-rust.wasm',
  python: 'tree-sitter-python.wasm',
  go: 'tree-sitter-go.wasm',
  java: 'tree-sitter-java.wasm',
  ruby: 'tree-sitter-ruby.wasm',
  cpp: 'tree-sitter-cpp.wasm',
  c: 'tree-sitter-c.wasm',
  json: 'tree-sitter-json.wasm',
  html: 'tree-sitter-html.wasm',
  css: 'tree-sitter-css.wasm',
  bash: 'tree-sitter-bash.wasm',
  php: 'tree-sitter-php.wasm',
};

let parser: any = null;
const loadedLanguages = new Map<string, any>();

/**
 * Initialize the TreeSitter parser
 */
export async function initParser(): Promise<void> {
  // Dynamic import - web-tree-sitter exports the Parser class
  const module = await import('web-tree-sitter');
  // Handle both ESM default export and CommonJS module.exports
  TreeSitter = module.default || module;

  await TreeSitter.init({
    locateFile: (scriptName: string) => `${import.meta.env.BASE_URL}grammars/${scriptName}`,
  });
  parser = new TreeSitter();
}

/**
 * Load a language grammar
 */
export async function loadLanguage(lang: string): Promise<any> {
  if (loadedLanguages.has(lang)) {
    return loadedLanguages.get(lang)!;
  }

  const grammarFile = GRAMMAR_FILES[lang];
  if (!grammarFile) {
    throw new Error(`Unsupported language: ${lang}`);
  }

  const language = await TreeSitter.Language.load(`${import.meta.env.BASE_URL}grammars/${grammarFile}`);
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
  parser.setLanguage(language);

  const tree = parser.parse(source);
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
  return Object.keys(GRAMMAR_FILES);
}
