import { Link } from 'react-router-dom';
import { DocLayout } from '../../components/DocLayout';
import { CodeBlock, OutputBlock } from '../../components/CodeBlock';

export function QueryCommand() {
  return (
    <DocLayout>
      <h1>query</h1>
      <p className="doc-lead">
        Explore and extract data from your code's syntax tree. This is the default command — just run <code>tractor</code> with a file.
      </p>

      <h2>Usage</h2>
      <CodeBlock code={`tractor [FILES] [OPTIONS]
tractor query [FILES] [OPTIONS]`} language="bash" />

      <h2>See the tree</h2>
      <p>
        Run tractor on a file to see its parsed structure. This is the first step — you see exactly what you're querying against.
      </p>
      <CodeBlock
        language="bash"
        title="View the tree of a C# file"
        code={`echo 'public class Greeter {\n    public string Greet(string name) {\n        return "Hello, " + name;\n    }\n}' | tractor -l csharp`}
      />
      <OutputBlock output={`<stdin>:1
<Files>
  <file>&lt;stdin&gt;</file>
  <unit>
    <class>
      <public/>
      class
      <name>Greeter</name>
      <body>
        {
        <method>
          <public/>
          <returns>
            <type>string</type>
          </returns>
          <name>Greet</name>
          <parameters>
            (
            <parameter>
              <type>string</type>
              <name>name</name>
            </parameter>
            )
          </parameters>
          <body>
            <block>
              {
              <return>
                return
                <binary>
                  <op>
                    <plus/>
                    +
                  </op>
                  <left>
                    <string>
                      &quot;
                      <string_literal_content>Hello, </string_literal_content>
                      &quot;
                    </string>
                  </left>
                  +
                  <right>
                    <ref>name</ref>
                  </right>
                </binary>
                ;
              </return>
              }
            </block>
          </body>
        </method>
        }
      </body>
    </class>
  </unit>
</Files>`} />

      <h2>Extract with -x</h2>
      <p>
        Use <code>-x</code> to query the tree and extract matching elements:
      </p>
      <CodeBlock
        language="bash"
        code={`echo 'public class Greeter {\n    public string Greet(string name) {\n        return "Hello, " + name;\n    }\n}' | tractor -l csharp -x "//class"`}
      />
      <p>This returns the full subtree of every <code>&lt;class&gt;</code> element.</p>

      <h2>Views</h2>
      <p>
        Use <code>-v</code> to control what you see. The default is <code>tree</code> (full XML), but other views are more useful for specific tasks.
      </p>

      <h3>value</h3>
      <p>Extract the text content of matched nodes:</p>
      <CodeBlock
        language="bash"
        code={`echo 'public class Greeter {\n    public string Greet(string name) {\n        return "Hello, " + name;\n    }\n}' | tractor -l csharp -x "//method/name" -v value`}
      />
      <OutputBlock output="Greet" />

      <h3>source</h3>
      <p>Get the exact source code of matched nodes:</p>
      <CodeBlock
        language="bash"
        code={`echo 'public class Greeter {\n    public string Greet(string name) {\n        return "Hello, " + name;\n    }\n}' | tractor -l csharp -x "//method" -v source`}
      />
      <OutputBlock output={`public string Greet(string name) {
        return "Hello, " + name;
    }`} />

      <h3>lines</h3>
      <p>Show the full source lines containing each match:</p>
      <CodeBlock
        language="bash"
        code={`echo 'public class Greeter {\n    public string Greet(string name) {\n        return "Hello, " + name;\n    }\n}' | tractor -l csharp -x "//method/name" -v lines`}
      />
      <OutputBlock output={`2 |     public string Greet(string name) {\n                      ^~~~~`} />

      <h3>count</h3>
      <p>Count matches:</p>
      <CodeBlock
        language="bash"
        code={`echo 'public class Foo { }\npublic class Bar { }\npublic class Baz { }' | tractor -l csharp -x "//class" -v count`}
      />
      <OutputBlock output="3" />

      <h3>schema</h3>
      <p>See the structural overview of element types. Learn more in the <Link to="/docs/guides/schema">Schema guide</Link>.</p>
      <CodeBlock
        language="bash"
        code={`echo 'public class Greeter {\n    public string Greet(string name) {\n        return "Hello, " + name;\n    }\n    public int Add(int a, int b) {\n        return a + b;\n    }\n}' | tractor -l csharp -x "//method" -v schema`}
      />
      <OutputBlock output={`method (2)
├─ public (2)
├─ returns (2)
│  └─ type (2)  string, int
├─ body (2)
│  └─ block (2)  {…}
│     └─ return (2)  return, ;
│        └─ binary (2)  +
│           └─ … (8 children)
├─ parameters (2)  (, ), ,
│  └─ parameter (3)
│     ├─ type (3)  string, int
│     └─ name (3)  name, a, b
└─ name (2)  Greet, Add

(use -d to increase depth, or -x to query specific elements)`} />

      <h2>Multi-language</h2>
      <p>Tractor works with 20+ languages. The same query syntax applies everywhere:</p>

      <h3>Python</h3>
      <CodeBlock
        language="bash"
        code={`echo 'def greet(name):\n    return f"Hello, {name}"\n\ndef add(a, b):\n    return a + b' | tractor -l python -x "//function/name" -v value`}
      />
      <OutputBlock output={`greet\nadd`} />

      <h3>Rust</h3>
      <CodeBlock
        language="bash"
        code={`echo 'fn main() {\n    println!("Hello");\n}\n\nfn add(a: i32, b: i32) -> i32 {\n    a + b\n}' | tractor -l rust -x "//function/name" -v value`}
      />
      <OutputBlock output={`main\nadd`} />

      <h3>TypeScript</h3>
      <CodeBlock
        language="bash"
        code={`echo 'export function greet(name: string): string {\n    return "Hello";\n}' | tractor -l typescript -x "//function/name" -v value`}
      />
      <OutputBlock output="greet" />

      <h3>JSON</h3>
      <CodeBlock
        language="bash"
        code={`echo '{"database": {"host": "localhost", "port": 5432}}' | tractor -l json -x "//database/host" -v value`}
      />
      <OutputBlock output="localhost" />

      <h2>Output Formats</h2>
      <p>Use <code>-f</code> to change the output format:</p>
      <table className="doc-table">
        <thead>
          <tr><th>Format</th><th>Description</th></tr>
        </thead>
        <tbody>
          <tr><td><code>text</code></td><td>Human-readable (default)</td></tr>
          <tr><td><code>json</code></td><td>JSON report</td></tr>
          <tr><td><code>yaml</code></td><td>YAML report</td></tr>
          <tr><td><code>xml</code></td><td>XML report</td></tr>
          <tr><td><code>gcc</code></td><td><code>file:line:col</code> format for editors</td></tr>
          <tr><td><code>github</code></td><td>GitHub Actions annotations</td></tr>
        </tbody>
      </table>

      <CodeBlock
        language="bash"
        title="JSON format"
        code={`echo 'public class Greeter {\n    public string Greet(string name) { return "Hello"; }\n}' | tractor -l csharp -x "//method/name" -v value -f json`}
      />
      <OutputBlock output={`{
  "results": [
    {
      "value": "Greet"
    }
  ]
}`} />

      <h2>Options Reference</h2>
      <table className="doc-table">
        <thead>
          <tr><th>Option</th><th>Description</th></tr>
        </thead>
        <tbody>
          <tr><td><code>-x, --extract</code></td><td>XPath expression to match</td></tr>
          <tr><td><code>-v, --view</code></td><td>View mode: tree, value, source, lines, count, schema</td></tr>
          <tr><td><code>-f, --format</code></td><td>Output format: text, json, yaml, xml, gcc, github</td></tr>
          <tr><td><code>-l, --lang</code></td><td>Language for stdin input</td></tr>
          <tr><td><code>-s, --string</code></td><td>Inline source code (alternative to stdin)</td></tr>
          <tr><td><code>-n, --limit</code></td><td>Limit output to first N matches</td></tr>
          <tr><td><code>-d, --depth</code></td><td>Limit tree output depth</td></tr>
          <tr><td><code>-t, --tree</code></td><td>Tree mode: raw, structure, data</td></tr>
          <tr><td><code>-W</code></td><td>Ignore whitespace in string matching</td></tr>
          <tr><td><code>-g, --group</code></td><td>Group output by file</td></tr>
          <tr><td><code>--diff-files</code></td><td>Only files changed in a git diff range</td></tr>
          <tr><td><code>--diff-lines</code></td><td>Only matches in changed hunks</td></tr>
        </tbody>
      </table>
    </DocLayout>
  );
}
