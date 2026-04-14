import { Link } from 'react-router-dom';
import { DocLayout } from '../../components/DocLayout';
import { CodeBlock } from '../../components/CodeBlock';

export function CheatSheet() {
  return (
    <DocLayout>
      <h1>Cheat Sheet</h1>
      <p className="doc-lead">
        Quick reference for tractor's query syntax and CLI. The query language is <a href="https://www.w3.org/TR/xpath-31/" target="_blank" rel="noopener noreferrer">XPath 3.1</a> — any XPath resource applies.
      </p>

      <h2>Path Expressions</h2>
      <table className="doc-table">
        <thead>
          <tr><th>Expression</th><th>Meaning</th></tr>
        </thead>
        <tbody>
          <tr><td><code>function</code></td><td>All <code>function</code> elements anywhere (implicit <code>//</code>)</td></tr>
          <tr><td><code>function/name</code></td><td>Direct <code>name</code> child of every function</td></tr>
          <tr><td><code>class//method</code></td><td>All methods anywhere inside a class</td></tr>
          <tr><td><code>class/body/method</code></td><td>Methods that are direct children of class body</td></tr>
          <tr><td><code>.</code></td><td>Current element (flattened text content)</td></tr>
          <tr><td><code>.//method</code></td><td>Any method descendant of current element</td></tr>
          <tr><td><code>parent::class</code></td><td>Parent class element</td></tr>
          <tr><td><code>ancestor::class</code></td><td>Nearest class ancestor</td></tr>
        </tbody>
      </table>

      <h2>Predicates</h2>
      <table className="doc-table">
        <thead>
          <tr><th>Predicate</th><th>Meaning</th></tr>
        </thead>
        <tbody>
          <tr><td><code>[public]</code></td><td>Has a <code>public</code> child element</td></tr>
          <tr><td><code>[not(static)]</code></td><td>Does not have a <code>static</code> child</td></tr>
          <tr><td><code>[name='Foo']</code></td><td>Has name equal to "Foo"</td></tr>
          <tr><td><code>[contains(name,'get')]</code></td><td>Name contains "get"</td></tr>
          <tr><td><code>[contains(.,'orderBy')]</code></td><td>Full text contains "orderBy"</td></tr>
          <tr><td><code>[starts-with(name,'test')]</code></td><td>Name starts with "test"</td></tr>
          <tr><td><code>[matches(name,'^[a-z]')]</code></td><td>Name matches regex</td></tr>
          <tr><td><code>[count(params/type) &gt; 3]</code></td><td>Has more than 3 parameters</td></tr>
          <tr><td><code>[string-length(name) &gt; 20]</code></td><td>Name is longer than 20 characters</td></tr>
          <tr><td><code>[position() = 1]</code></td><td>First match only</td></tr>
        </tbody>
      </table>

      <h2>Combining Predicates</h2>
      <table className="doc-table">
        <thead>
          <tr><th>Pattern</th><th>Meaning</th></tr>
        </thead>
        <tbody>
          <tr><td><code>[public][not(static)]</code></td><td>AND — both must be true (chained)</td></tr>
          <tr><td><code>[public and not(static)]</code></td><td>AND — both must be true (single predicate)</td></tr>
          <tr><td><code>[public or static]</code></td><td>OR — either is true</td></tr>
          <tr><td><code>[not(public or static)]</code></td><td>NOR — neither is true</td></tr>
        </tbody>
      </table>

      <h2>Functions</h2>
      <table className="doc-table">
        <thead>
          <tr><th>Function</th><th>Description</th></tr>
        </thead>
        <tbody>
          <tr><td><code>contains(a, b)</code></td><td>String <code>a</code> contains <code>b</code></td></tr>
          <tr><td><code>starts-with(a, b)</code></td><td>String <code>a</code> starts with <code>b</code></td></tr>
          <tr><td><code>ends-with(a, b)</code></td><td>String <code>a</code> ends with <code>b</code></td></tr>
          <tr><td><code>matches(a, regex)</code></td><td>String <code>a</code> matches regex</td></tr>
          <tr><td><code>not(expr)</code></td><td>Negates a condition</td></tr>
          <tr><td><code>count(nodes)</code></td><td>Count matching nodes</td></tr>
          <tr><td><code>string-length(s)</code></td><td>Length of a string</td></tr>
          <tr><td><code>concat(a, b, ...)</code></td><td>Concatenate strings</td></tr>
          <tr><td><code>translate(s, from, to)</code></td><td>Character-by-character replacement</td></tr>
          <tr><td><code>normalize-space(s)</code></td><td>Collapse whitespace</td></tr>
          <tr><td><code>string(node)</code></td><td>Convert node to string</td></tr>
        </tbody>
      </table>

      <h2>Maps &amp; Arrays</h2>
      <p>
        Build structured output with <a href="https://www.w3.org/TR/xpath-31/#id-maps-and-arrays" target="_blank" rel="noopener noreferrer">XPath 3.1 maps and arrays</a>. Useful with <code>-f json</code> or <code>-f yaml</code>.
      </p>
      <table className="doc-table">
        <thead>
          <tr><th>Syntax</th><th>Result</th></tr>
        </thead>
        <tbody>
          <tr><td><code>{'map { "k": "v" }'}</code></td><td>{'{ "k": "v" }'}</td></tr>
          <tr><td><code>{'map { "name": string(name), "line": string(@line) }'}</code></td><td>Map with computed values</td></tr>
          <tr><td><code>{'array { "a", "b", "c" }'}</code></td><td>{'["a", "b", "c"]'}</td></tr>
          <tr><td><code>{'array { //function/name/string(.) }'}</code></td><td>Array from query results</td></tr>
        </tbody>
      </table>
      <h3>Example: extract structured data</h3>
      <CodeBlock language="bash" code={`# Build a JSON array of objects from code
tractor file.cs -f json -x '//class ! map {
  "name": string(name),
  "methods": array { body/method/name/string(.) }
}'`} />
      <p>
        When a map value produces multiple items, tractor automatically wraps them in an array.
      </p>

      <h2>Axes</h2>
      <table className="doc-table">
        <thead>
          <tr><th>Axis</th><th>Description</th></tr>
        </thead>
        <tbody>
          <tr><td><code>child::</code></td><td>Direct children (default, same as <code>/</code>)</td></tr>
          <tr><td><code>descendant::</code></td><td>All descendants (same as <code>//</code>)</td></tr>
          <tr><td><code>parent::</code></td><td>Parent element</td></tr>
          <tr><td><code>ancestor::</code></td><td>All ancestors up to root</td></tr>
          <tr><td><code>following-sibling::</code></td><td>Siblings after this element</td></tr>
          <tr><td><code>preceding-sibling::</code></td><td>Siblings before this element</td></tr>
          <tr><td><code>self::</code></td><td>Current element (with type test)</td></tr>
        </tbody>
      </table>

      <h2>Views (<code>-v</code>)</h2>
      <table className="doc-table">
        <thead>
          <tr><th>View</th><th>Shows</th></tr>
        </thead>
        <tbody>
          <tr><td><code>tree</code></td><td>Tree structure (default)</td></tr>
          <tr><td><code>value</code></td><td>Text content of matched nodes</td></tr>
          <tr><td><code>source</code></td><td>Exact source code</td></tr>
          <tr><td><code>lines</code></td><td>Full source lines with markers</td></tr>
          <tr><td><code>count</code></td><td>Number of matches</td></tr>
          <tr><td><code>schema</code></td><td>Structural overview of element types</td></tr>
        </tbody>
      </table>

      <h2>Output Formats (<code>-f</code>)</h2>
      <table className="doc-table">
        <thead>
          <tr><th>Format</th><th>Use case</th></tr>
        </thead>
        <tbody>
          <tr><td><code>text</code></td><td>Human-readable (default for query)</td></tr>
          <tr><td><code>gcc</code></td><td><code>file:line:col</code> for editors and CI (default for check)</td></tr>
          <tr><td><code>json</code></td><td>Machine-readable reports</td></tr>
          <tr><td><code>yaml</code></td><td>Machine-readable reports</td></tr>
          <tr><td><code>github</code></td><td>GitHub Actions annotations</td></tr>
          <tr><td><code>claude-code</code></td><td>Claude Code hook format</td></tr>
        </tbody>
      </table>

      <h2>Common Recipes</h2>
      <CodeBlock language="bash" code={`# Explore the tree
tractor file.js                          # see tree structure
tractor file.js -v schema                # see element types
tractor file.js -x "//function" -v schema  # zoom into functions

# Extract
tractor file.js -x "//function/name" -v value      # function names
tractor file.js -x "//class" -v source              # full class source
tractor file.js -x "//function" -v count            # count functions

# Filter
tractor file.js -x "//method[public][not(static)]/name" -v value
tractor file.js -x "//method[contains(name,'get')]/name" -v value
tractor file.js -x "//function[count(parameters//type) > 5]/name" -v value

# Check (lint)
tractor check "src/**/*.js" -x "//comment[contains(.,'TODO')]" \\
    --reason "Resolve TODOs before merging"

# Set (modify)
tractor set config.json -x "//database/host" --value "localhost"

# Multi-file
tractor "src/**/*.js" -x "//function/name" -v value
tractor "src/**/*.js" --diff-lines "main..HEAD" -x "//function" -v count`} />

      <h2>Variables</h2>
      <table className="doc-table">
        <thead>
          <tr><th>Variable</th><th>Description</th></tr>
        </thead>
        <tbody>
          <tr><td><code>$file</code></td><td>Path of the current file being queried</td></tr>
        </tbody>
      </table>

      <h2>Key CLI Flags</h2>
      <table className="doc-table">
        <thead>
          <tr><th>Flag</th><th>Description</th></tr>
        </thead>
        <tbody>
          <tr><td><code>-x</code></td><td>Query to match</td></tr>
          <tr><td><code>-v</code></td><td>View mode (tree, value, source, lines, count, schema)</td></tr>
          <tr><td><code>-f</code></td><td>Output format (text, gcc, json, yaml, github)</td></tr>
          <tr><td><code>-d</code></td><td>Limit tree depth</td></tr>
          <tr><td><code>-n</code></td><td>Limit number of matches</td></tr>
          <tr><td><code>-W</code></td><td>Ignore whitespace in string matching</td></tr>
          <tr><td><code>-t</code></td><td>Tree mode (structure, data, raw)</td></tr>
          <tr><td><code>--meta</code></td><td>Include position and kind metadata</td></tr>
          <tr><td><code>--diff-files</code></td><td>Only files changed in a git range</td></tr>
          <tr><td><code>--diff-lines</code></td><td>Only matches in changed hunks</td></tr>
        </tbody>
      </table>

      <h2>More Resources</h2>
      <ul>
        <li><Link to="/docs/guides/query-syntax">Query Syntax</Link> — full guide with examples and explanations</li>
        <li><Link to="/docs/guides/writing-queries">Writing Queries</Link> — step-by-step tutorial</li>
        <li><Link to="/docs/reference/cli">CLI Reference</Link> — every option with examples</li>
        <li><a href="https://devhints.io/xpath" target="_blank" rel="noopener noreferrer">XPath Cheat Sheet</a> — quick reference for the underlying query language</li>
        <li><a href="https://www.w3.org/TR/xpath-31/" target="_blank" rel="noopener noreferrer">XPath 3.1 Specification</a> — complete W3C spec</li>
      </ul>
    </DocLayout>
  );
}
