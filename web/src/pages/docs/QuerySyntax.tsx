import { Link } from 'react-router-dom';
import { DocLayout } from '../../components/DocLayout';
import { CodeBlock, Example } from '../../components/CodeBlock';

const GREETER_JS = `function greet(name) {
  return "Hello, " + name;
}

function add(a, b) {
  return a + b;
}`;

const USER_SERVICE_JS = `class UserService {
  static findById(id) {
    return null;
  }

  save(user) {
    return user;
  }

  _log(msg) {
    console.log(msg);
  }
}`;

export function QuerySyntax() {
  return (
    <DocLayout>
      <h1>Query Syntax</h1>
      <p className="doc-lead">
        Tractor uses <a href="https://www.w3.org/TR/xpath-31/" target="_blank" rel="noopener noreferrer">path expressions</a> to query code. Just name the element you want — tractor searches the whole tree. Use <code>/</code> to navigate to children, <code>//</code> to search deeper.
      </p>

      <h2>Path Basics</h2>

      <h3>Searching by name (the default)</h3>
      <p>
        When you write <code>-x "function"</code>, tractor searches the entire tree for elements named <code>function</code>. This is the same as <code>//function</code> — tractor adds the <code>//</code> implicitly.
      </p>
      <p>
        This is the most common way to query. You almost never need to write <code>//</code> explicitly at the start.
      </p>

      <Example
        file={{ name: 'greeter.js', language: 'js', content: GREETER_JS }}
        command={`tractor greeter.js -x "function/name" -v value`}
        output={`greet\nadd`}
      />
      <p>
        Here <code>function</code> finds all functions anywhere in the tree, and <code>/name</code> selects their direct <code>name</code> child.
      </p>

      <h3>/ — Direct child</h3>
      <p>
        A single slash selects direct children. <code>function/name</code> means "the <code>name</code> that is a direct child of <code>function</code>."
      </p>

      <h3>// — Descendant (search deeper)</h3>
      <p>
        A double slash searches all descendants, not just direct children. <code>class//method</code> finds methods anywhere inside a class, even if they're nested inside a <code>body</code> element.
      </p>

      <h3>Combining / and //</h3>
      <table className="doc-table">
        <thead>
          <tr><th>Expression</th><th>Meaning</th></tr>
        </thead>
        <tbody>
          <tr><td><code>function</code></td><td>All functions anywhere (implicit <code>//</code>)</td></tr>
          <tr><td><code>function/name</code></td><td>The name of every function</td></tr>
          <tr><td><code>class//method</code></td><td>All methods anywhere inside a class</td></tr>
          <tr><td><code>class/body/method</code></td><td>Methods that are direct children of a class body</td></tr>
          <tr><td><code>function/parameters//type</code></td><td>All types anywhere in function parameters</td></tr>
        </tbody>
      </table>

      <h2>Predicates — Filtering Matches</h2>
      <p>
        Predicates go in square brackets after an element name to filter results. They answer the question: "which ones?"
      </p>

      <h3>Check for a child element</h3>
      <p>
        Tractor's tree uses empty marker elements for modifiers like <code>public</code>, <code>static</code>, or <code>async</code>. You query them by just naming them:
      </p>
      <Example
        file={{ name: 'user-service.js', language: 'js', content: USER_SERVICE_JS }}
        command={`tractor user-service.js -x "//method" -v schema`}
        output={`method (3)  static
├─ body (3)
│  └─ block (3)  {, }, ;
  }
│     ├─ call
│     │  ├─ arguments
│     │  │  └─ … (2 children)
│     │  └─ function
│     │     └─ … (5 children)
│     └─ return (2)  return, ;
│        ├─ null  null
│        └─ type  user
├─ name (3)  findById, save, _log
└─ parameters (3)
   └─ params (3)  (…)
      └─ type (3)  id, user, msg

(use -d to increase depth, or -x to query specific elements)`}
      />
      <p>
        You can see <code>static</code> appears as a child of <code>method</code>. To filter:
      </p>
      <Example
        command={`tractor user-service.js -x "//method[static]/name" -v value`}
        output="findById"
      />
      <Example
        command={`tractor user-service.js -x "//method[not(static)]/name" -v value`}
        output={`save\n_log`}
      />

      <h3>Check text content with contains()</h3>
      <p>
        Use <code>contains()</code> to match against the text content of an element. Here we find the method whose name contains "find" and return its source:
      </p>
      <Example
        command={`tractor user-service.js -x "//method[contains(name,'find')]" -v source`}
        output={`static findById(id) {
    return null;
  }`}
      />
      <p>
        You can also write this the other way — filter the name itself with the dot (<code>.</code>), which refers to the text of the current node:
      </p>
      <CodeBlock language="bash" code={`# Same result, different style
tractor user-service.js -x "//method/name[contains(.,'find')]" -v value`} />

      <h3>Exact match with =</h3>
      <CodeBlock language="bash" code={`tractor user-service.js -x "//method[name='save']" -v source`} />

      <h3>The dot (.) — Current node's text</h3>
      <p>
        The dot <code>.</code> refers to the full text content of the current element — all nested text concatenated together. This is powerful because it lets you match against the source code as a flat string:
      </p>
      <CodeBlock language="bash" code={`# Find methods that call console.log
tractor user-service.js -x "//method[contains(.,'console.log')]/name" -v value`} />
      <p>
        Even though <code>console.log(msg)</code> is represented as nested elements in the tree (<code>call</code>, <code>function</code>, <code>arguments</code>, etc.), the dot flattens all the text together and matches <code>"console.log"</code> against it.
      </p>

      <h3>Negation with not()</h3>
      <CodeBlock language="bash" code={`# Methods that do NOT contain a return statement
tractor file.js -x "//method[not(.//return)]/name" -v value

# Methods without a specific child element
tractor file.js -x "//method[not(static)]/name" -v value`} />

      <h2>Common Functions</h2>
      <table className="doc-table">
        <thead>
          <tr><th>Function</th><th>Description</th><th>Example</th></tr>
        </thead>
        <tbody>
          <tr><td><code>contains(a, b)</code></td><td>String a contains b</td><td><code>[contains(name,'get')]</code></td></tr>
          <tr><td><code>starts-with(a, b)</code></td><td>String a starts with b</td><td><code>[starts-with(name,'test')]</code></td></tr>
          <tr><td><code>not(expr)</code></td><td>Negates a condition</td><td><code>[not(static)]</code></td></tr>
          <tr><td><code>count(nodes)</code></td><td>Count matching nodes</td><td><code>[count(parameters//type) &gt; 3]</code></td></tr>
          <tr><td><code>string-length(s)</code></td><td>Length of a string</td><td><code>[string-length(name) &gt; 20]</code></td></tr>
        </tbody>
      </table>

      <h2>Subqueries (Predicates with Paths)</h2>
      <p>
        Inside a predicate <code>[...]</code>, you can write path expressions. These are always relative to the current element — no prefix needed.
      </p>

      <h3>Paths inside predicates are relative</h3>
      <p>
        A path like <code>body/method</code> inside a predicate means: "the current element has a child <code>body</code> which has a child <code>method</code>." It works just like a regular path, starting from the matched element:
      </p>
      <Example
        file={{ name: 'service.js', language: 'js', content: `class UserService {
  save(user) { return user; }
  delete(id) { return null; }
}

class Logger {
  log(msg) { console.log(msg); }
}` }}
        command={`tractor service.js -x "//class[body/method/name='save']/name" -v value`}
        output="UserService"
      />
      <p>
        This works because the tree structure is <code>class &gt; body &gt; method &gt; name</code>. The predicate walks that path relative to the <code>class</code> element.
      </p>

      <h3>Watch out: // in predicates searches globally</h3>
      <p>
        <strong>Pitfall:</strong> <code>//</code> inside a predicate searches from the <em>root</em> of the document, not from the current element:
      </p>
      <Example
        command={`# BUG: returns ALL classes, because //method searches the whole document!
tractor service.js -x "//class[//method/name='save']/name" -v value`}
        output={`UserService\nLogger`}
      />
      <p>
        Both classes match because <code>//method/name='save'</code> finds the <code>save</code> method anywhere in the document. Since it exists somewhere, the condition is true for every class. To search descendants of the current element, use <code>.//</code> (with a dot):
      </p>
      <Example
        command={`# CORRECT: .// searches inside the current class only
tractor service.js -x "//class[.//method/name='save']/name" -v value`}
        output="UserService"
      />

      <h3>Quick reference</h3>
      <table className="doc-table">
        <thead>
          <tr><th>Inside <code>[...]</code></th><th>Searches</th><th>Example</th></tr>
        </thead>
        <tbody>
          <tr><td><code>name</code></td><td>Direct children of current element</td><td><code>[name='save']</code></td></tr>
          <tr><td><code>body/method</code></td><td>Specific path from current element</td><td><code>[body/method/name='save']</code></td></tr>
          <tr><td><code>.//method</code></td><td>Any descendant of current element</td><td><code>[.//method/name='save']</code></td></tr>
          <tr><td><code>//method</code></td><td>Anywhere in the whole document</td><td>Usually a mistake in predicates</td></tr>
        </tbody>
      </table>

      <h3>More examples</h3>
      <CodeBlock language="bash" code={`# Functions where any parameter has type 'string'
tractor file.js -x "//function[parameters//type='string']/name" -v value

# Methods with more than 3 parameters
tractor file.js -x "//method[count(parameters//type) > 3]/name" -v value

# Classes that have a static method
tractor file.js -x "//class[.//method[static]]/name" -v value`} />

      <h2>Multiple Predicates</h2>
      <p>
        Chain predicates to combine conditions (logical AND):
      </p>
      <CodeBlock language="bash" code={`# Public, non-static methods
tractor file.js -x "//method[public][not(static)]/name" -v value

# Methods named 'getAll' that don't contain 'orderBy'
tractor file.js -x "//method[contains(name,'getAll')][not(contains(.,'orderBy'))]/name" -v value`} />

      <h2>Axes</h2>
      <p>
        Axes let you navigate the tree in different directions:
      </p>
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
        </tbody>
      </table>
      <CodeBlock language="bash" code={`# Find the class that contains a method named 'save'
tractor file.js -x "//method[name='save']/ancestor::class/name" -v value`} />

      <h2>Context Variables</h2>
      <p>
        Tractor provides built-in variables you can use in any expression:
      </p>
      <table className="doc-table">
        <thead>
          <tr><th>Variable</th><th>Type</th><th>Description</th></tr>
        </thead>
        <tbody>
          <tr><td><code>$file</code></td><td>string</td><td>Path of the current file being queried</td></tr>
        </tbody>
      </table>

      <h3>Using $file in predicates</h3>
      <p>
        The <code>$file</code> variable lets you write queries that cross-reference file paths with AST content.
        For example, detecting C# files where the namespace doesn't match the directory structure:
      </p>
      <CodeBlock language="bash" code={`# Namespace MyApp.Services should be in a MyApp/Services/ directory
# translate() converts dots to slashes: MyApp.Services → MyApp/Services
tractor "src/**/*.cs" -x "//namespace[not(contains($file, translate(string(name), '.', '/')))]"

# Simply check the file path
tractor "src/**/*.cs" -x "$file" -v value`} />

      <h2>Tips</h2>
      <ul>
        <li><strong>Start with <code>//</code></strong> — you almost never need full paths from root.</li>
        <li><strong>Use <code>-v schema</code></strong> to discover element names before writing queries.</li>
        <li><strong>The dot <code>.</code> is your friend</strong> — <code>contains(.,'text')</code> matches against the flattened source code of any element.</li>
        <li><strong>No attributes</strong> — tractor models everything as elements and text. You won't need <code>@attr</code> syntax.</li>
        <li><strong>AI tools know the syntax</strong> — ChatGPT and Claude can write tractor queries. Show them the schema output and ask for a query.</li>
        <li><strong>Full reference</strong> — the query language is XPath 3.1. See the <a href="https://devhints.io/xpath" target="_blank" rel="noopener noreferrer">XPath cheat sheet</a> for a quick overview or the <a href="https://www.w3.org/TR/xpath-31/" target="_blank" rel="noopener noreferrer">W3C spec</a> for the complete reference.</li>
      </ul>

      <div className="doc-next">
        <p>Next: <Link to="/docs/guides/writing-queries">Writing Queries</Link> — a step-by-step tutorial for building queries iteratively.</p>
      </div>
    </DocLayout>
  );
}
