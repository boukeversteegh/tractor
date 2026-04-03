import { Link } from 'react-router-dom';
import { DocLayout } from '../../components/DocLayout';
import { CodeBlock, Example } from '../../components/CodeBlock';

export function CodeLanguages() {
  return (
    <DocLayout>
      <h1>Code Languages</h1>
      <p className="doc-lead">
        Tractor supports 20+ programming languages. Code is parsed into a tree with semantic elements like <code>function</code>, <code>class</code>, <code>method</code>, and <code>parameters</code>.
      </p>

      <h2>Supported Languages</h2>
      <p>
        Languages with <strong>full</strong> support have a semantic transform layer that renames nodes, extracts modifiers (like <code>public</code> and <code>static</code>), and structures the tree for intuitive querying. Languages with <strong>basic</strong> support use the raw parser output directly — still queryable, but element names follow the parser's conventions.
      </p>
      <table className="doc-table">
        <thead>
          <tr><th>Language</th><th>Extension</th><th>-l value</th><th>Support</th></tr>
        </thead>
        <tbody>
          <tr><td>C#</td><td><code>.cs</code></td><td><code>csharp</code></td><td><span className="badge badge-full">Full</span></td></tr>
          <tr><td>JavaScript</td><td><code>.js</code></td><td><code>javascript</code></td><td><span className="badge badge-full">Full</span></td></tr>
          <tr><td>TypeScript</td><td><code>.ts</code></td><td><code>typescript</code></td><td><span className="badge badge-full">Full</span></td></tr>
          <tr><td>TSX</td><td><code>.tsx</code></td><td><code>tsx</code></td><td><span className="badge badge-full">Full</span></td></tr>
          <tr><td>Python</td><td><code>.py</code></td><td><code>python</code></td><td><span className="badge badge-full">Full</span></td></tr>
          <tr><td>Go</td><td><code>.go</code></td><td><code>go</code></td><td><span className="badge badge-full">Full</span></td></tr>
          <tr><td>Java</td><td><code>.java</code></td><td><code>java</code></td><td><span className="badge badge-full">Full</span></td></tr>
          <tr><td>Rust</td><td><code>.rs</code></td><td><code>rust</code></td><td><span className="badge badge-full">Full</span></td></tr>
          <tr><td>T-SQL</td><td><code>.sql</code></td><td><code>tsql</code></td><td><span className="badge badge-full">Full</span></td></tr>
          <tr><td>Ruby</td><td><code>.rb</code></td><td><code>ruby</code></td><td><span className="badge badge-good">Good</span></td></tr>
          <tr><td>C</td><td><code>.c</code></td><td><code>c</code></td><td><span className="badge badge-basic">Basic</span></td></tr>
          <tr><td>C++</td><td><code>.cpp</code></td><td><code>cpp</code></td><td><span className="badge badge-basic">Basic</span></td></tr>
          <tr><td>Bash</td><td><code>.sh</code></td><td><code>bash</code></td><td><span className="badge badge-basic">Basic</span></td></tr>
          <tr><td>PHP</td><td><code>.php</code></td><td><code>php</code></td><td><span className="badge badge-basic">Basic</span></td></tr>
          <tr><td>Scala</td><td><code>.scala</code></td><td><code>scala</code></td><td><span className="badge badge-basic">Basic</span></td></tr>
          <tr><td>Lua</td><td><code>.lua</code></td><td><code>lua</code></td><td><span className="badge badge-basic">Basic</span></td></tr>
          <tr><td>Haskell</td><td><code>.hs</code></td><td><code>haskell</code></td><td><span className="badge badge-basic">Basic</span></td></tr>
          <tr><td>OCaml</td><td><code>.ml</code></td><td><code>ocaml</code></td><td><span className="badge badge-basic">Basic</span></td></tr>
          <tr><td>R</td><td><code>.r</code></td><td><code>r</code></td><td><span className="badge badge-basic">Basic</span></td></tr>
          <tr><td>Julia</td><td><code>.jl</code></td><td><code>julia</code></td><td><span className="badge badge-basic">Basic</span></td></tr>
        </tbody>
      </table>
      <p>
        <strong>Full</strong> — Semantic transforms: nodes renamed to intuitive names (<code>function</code>, <code>method</code>, <code>class</code>), modifiers extracted as marker elements (<code>[public]</code>, <code>[static]</code>), operators and accessors normalized.<br/>
        <strong>Good</strong> — Partial transforms: common patterns covered, some raw parser names remain.<br/>
        <strong>Basic</strong> — Raw parser output with minimal cleanup. Still fully queryable — use <code>-v schema</code> to discover element names. Contributions welcome.
      </p>

      <h2>How Code Trees Work</h2>
      <p>
        Code is parsed into a semantic tree. Tractor applies transforms to make the tree more intuitive — for example, turning raw parser nodes into meaningful names like <code>function</code>, <code>class</code>, and <code>method</code>.
      </p>
      <p>
        The best way to learn what's in the tree is to look at it:
      </p>
      <Example
        file={{ name: 'greeter.js', language: 'js', content: `function greet(name) {
  return "Hello, " + name;
}

function add(a, b) {
  return a + b;
}` }}
        command={`tractor greeter.js -x "//function" -v schema`}
        output={`function (2)  function
├─ body (2)
│  └─ block (2)  {…}
│     └─ return (2)  return, ;
│        └─ binary (2)  +
│           └─ … (8 children)
├─ parameters (2)
│  └─ params (2)  (, ), ,
│     └─ type (3)  name, a, b
└─ name (2)  greet, add

(use -d to increase depth, or -x to query specific elements)`}
      />

      <h3>Modifiers are marker elements</h3>
      <p>
        Access modifiers like <code>public</code>, <code>static</code>, <code>async</code> are represented as empty child elements. This makes querying natural:
      </p>
      <CodeBlock language="bash" code={`# Find public methods
tractor "src/**/*.cs" -x "//method[public]/name" -v value

# Find static methods
tractor "src/**/*.js" -x "//method[static]/name" -v value

# Find non-static methods
tractor "src/**/*.js" -x "//method[not(static)]/name" -v value`} />

      <h3>Text content matches source code</h3>
      <p>
        When you compare an element's text with a string, tractor flattens all the nested elements and compares against the source code as written. This means <code>contains(.,'console.log')</code> works even though the function call is represented as multiple nested elements in the tree.
      </p>

      <h2>Language Auto-detection</h2>
      <p>
        When processing files, tractor detects the language from the file extension. No configuration needed:
      </p>
      <CodeBlock language="bash" code={`# Auto-detected from extension
tractor greeter.js -x "//function/name" -v value
tractor main.rs -x "//function/name" -v value
tractor app.cs -x "//method/name" -v value

# Mix languages in one command
tractor "src/**/*.js" "src/**/*.ts" -x "//function/name" -v value`} />
      <p>
        When reading from stdin, use <code>-l</code> to specify the language:
      </p>
      <CodeBlock language="bash" code={`echo 'fn main() {}' | tractor -l rust -x "//function/name" -v value`} />

      <h2>Bash</h2>
      <Example
        file={{ name: 'deploy.sh', language: 'bash', content: `#!/bin/bash
if [ -f "file.txt" ]; then
  echo "found"
fi
for i in 1 2 3; do
  echo $i
done` }}
        command={`tractor deploy.sh -v schema -d 5`}
        output={`Files
└─ File
   └─ program
      ├─ for_statement  for, in, ;
      │  ├─ variable_name  i
      │  ├─ value (3)
      │  │  └─ number (3)  1, 2, 3
      │  └─ body
      │     └─ do_group  do, done
      │        └─ … (6 children)
      ├─ if_statement  if, ; then, fi
      │  ├─ condition
      │  │  └─ test_command  […]
      │  │     └─ … (4 children)
      │  └─ command
      │     ├─ name
      │     │  └─ … (2 children)
      │     └─ string  "
      │        └─ … (1 children)
      └─ comment  #!/bin/bash

(use -d to increase depth, or -x to query specific elements)`}
      />

      <h2>Discovering the Tree</h2>
      <p>
        Every language produces a different tree. The workflow is always the same:
      </p>
      <ol>
        <li>Run <code>tractor file -v schema</code> to see the top-level structure</li>
        <li>Zoom in with <code>-x "//element" -v schema</code></li>
        <li>Increase depth with <code>-d 6</code> or <code>-d 8</code> to see deeper elements</li>
        <li>Once you know the element names, write your query</li>
      </ol>
      <p>
        See the <Link to="/docs/guides/writing-queries">Writing Queries</Link> guide for a step-by-step tutorial.
      </p>
    </DocLayout>
  );
}
