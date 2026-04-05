/**
 * Lightweight regex-based syntax highlighting for documentation code blocks.
 * Uses the same CSS classes as the tractor ANSI output (.hl-keyword, etc.)
 */

function escapeHtml(text: string): string {
  return text
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;');
}

type Rule = [RegExp, string];

function applyRules(code: string, rules: Rule[]): string {
  // Tokenize: find all matches, sort by position, avoid overlaps
  const tokens: { start: number; end: number; cls: string; text: string }[] = [];

  for (const [pattern, cls] of rules) {
    const re = new RegExp(pattern.source, pattern.flags.includes('g') ? pattern.flags : pattern.flags + 'g');
    let m;
    while ((m = re.exec(code)) !== null) {
      tokens.push({ start: m.index, end: m.index + m[0].length, cls, text: m[0] });
    }
  }

  // Sort by start position, longer matches first
  tokens.sort((a, b) => a.start - b.start || b.end - a.end);

  // Build output, skipping overlaps
  let result = '';
  let pos = 0;

  for (const token of tokens) {
    if (token.start < pos) continue; // overlapping
    if (token.start > pos) {
      result += escapeHtml(code.slice(pos, token.start));
    }
    result += `<span class="${token.cls}">${escapeHtml(token.text)}</span>`;
    pos = token.end;
  }

  if (pos < code.length) {
    result += escapeHtml(code.slice(pos));
  }

  return result;
}

const JAVASCRIPT_RULES: Rule[] = [
  [/\/\/.*$/gm, 'hl-comment'],
  [/\/\*[\s\S]*?\*\//g, 'hl-comment'],
  [/"(?:[^"\\]|\\.)*"|'(?:[^'\\]|\\.)*'|`(?:[^`\\]|\\.)*`/g, 'hl-string'],
  [/\b\d+\.?\d*\b/g, 'hl-number'],
  [/\b(?:function|return|class|const|let|var|if|else|for|while|do|switch|case|break|continue|new|this|import|export|from|default|static|async|await|throw|try|catch|finally|typeof|instanceof|null|undefined|true|false)\b/g, 'hl-keyword'],
];

const TYPESCRIPT_RULES: Rule[] = [
  ...JAVASCRIPT_RULES.slice(0, 4),
  [/\b(?:function|return|class|const|let|var|if|else|for|while|do|switch|case|break|continue|new|this|import|export|from|default|static|async|await|throw|try|catch|finally|typeof|instanceof|null|undefined|true|false|type|interface|enum|implements|extends|public|private|protected|readonly|as|is|keyof|infer)\b/g, 'hl-keyword'],
  [/:\s*([A-Z]\w*)/g, 'hl-type'],
];

const RUST_RULES: Rule[] = [
  [/\/\/.*$/gm, 'hl-comment'],
  [/"(?:[^"\\]|\\.)*"/g, 'hl-string'],
  [/\b\d+\.?\d*\b/g, 'hl-number'],
  [/\b(?:fn|let|mut|pub|struct|impl|enum|trait|use|mod|crate|self|super|return|if|else|for|while|loop|match|break|continue|async|await|move|ref|where|type|const|static|true|false|Some|None|Ok|Err)\b/g, 'hl-keyword'],
  [/\b(?:i32|i64|u32|u64|f32|f64|bool|str|String|Vec|Option|Result|Box|usize|isize)\b/g, 'hl-type'],
];

const YAML_RULES: Rule[] = [
  [/#.*$/gm, 'hl-comment'],
  [/"(?:[^"\\]|\\.)*"|'(?:[^'\\]|\\.)*'/g, 'hl-string'],
  [/^[\w][\w\s-]*(?=:)/gm, 'hl-key'],
  [/^\s+[\w][\w-]*(?=:)/gm, 'hl-key'],
  [/\b(?:true|false|null|yes|no)\b/g, 'hl-keyword'],
  [/\b\d+\.?\d*\b/g, 'hl-number'],
  [/>-?\s*$/gm, 'hl-punctuation'],
];

const JSON_RULES: Rule[] = [
  [/"(?:[^"\\]|\\.)*"\s*(?=:)/g, 'hl-key'],
  [/"(?:[^"\\]|\\.)*"/g, 'hl-string'],
  [/\b\d+\.?\d*\b/g, 'hl-number'],
  [/\b(?:true|false|null)\b/g, 'hl-keyword'],
];

const BASH_RULES: Rule[] = [
  [/#.*$/gm, 'hl-comment'],
  [/"(?:[^"\\]|\\.)*"|'(?:[^'\\]|\\.)*'/g, 'hl-string'],
  [/\b(?:if|then|else|elif|fi|for|while|do|done|case|esac|function|return|exit|echo|cd|chmod|curl|sudo|mv|cp|rm|mkdir|cat|grep|export|source|local|set|unset)\b/g, 'hl-keyword'],
  [/\b(?:tractor|npm|cargo|git)\b/g, 'hl-type'],
  [/--?[\w-]+/g, 'hl-flag'],
  [/\$\{?[\w]+\}?/g, 'hl-variable'],
  [/\||\|\||&&|;|>/g, 'hl-punctuation'],
];

const XML_RULES: Rule[] = [
  [/<!--[\s\S]*?-->/g, 'hl-comment'],
  [/&\w+;/g, 'hl-string'],
  [/<\/?\w+[\w-]*/g, 'hl-tag'],
  [/\/?>/g, 'hl-tag'],
  [/\b\w+(?==)/g, 'hl-key'],
  [/"(?:[^"\\]|\\.)*"/g, 'hl-string'],
];

const CSHARP_RULES: Rule[] = [
  [/\/\/.*$/gm, 'hl-comment'],
  [/\/\*[\s\S]*?\*\//g, 'hl-comment'],
  [/"(?:[^"\\]|\\.)*"|'(?:[^'\\]|\\.)*'/g, 'hl-string'],
  [/\b\d+\.?\d*\b/g, 'hl-number'],
  [/\b(?:public|private|protected|internal|static|class|interface|struct|enum|void|return|if|else|for|foreach|while|do|switch|case|break|continue|new|this|base|using|namespace|var|const|readonly|abstract|virtual|override|sealed|async|await|throw|try|catch|finally|null|true|false|string|int|bool|double|float|long|byte|object|dynamic|partial|get|set)\b/g, 'hl-keyword'],
  [/\b[A-Z]\w*\b/g, 'hl-type'],
];

const LANGUAGE_RULES: Record<string, Rule[]> = {
  javascript: JAVASCRIPT_RULES,
  js: JAVASCRIPT_RULES,
  typescript: TYPESCRIPT_RULES,
  ts: TYPESCRIPT_RULES,
  rust: RUST_RULES,
  yaml: YAML_RULES,
  yml: YAML_RULES,
  json: JSON_RULES,
  bash: BASH_RULES,
  sh: BASH_RULES,
  xml: XML_RULES,
  csharp: CSHARP_RULES,
  cs: CSHARP_RULES,
};

export function highlightCode(code: string, language?: string): string {
  if (!language) return escapeHtml(code);
  const rules = LANGUAGE_RULES[language.toLowerCase()];
  if (!rules) return escapeHtml(code);
  return applyRules(code, rules);
}
