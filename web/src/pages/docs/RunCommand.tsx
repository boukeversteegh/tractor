import { Link } from 'react-router-dom';
import { DocLayout } from '../../components/DocLayout';
import { CodeBlock, OutputBlock, Example } from '../../components/CodeBlock';

const EXAMPLE_JS = `// TODO: fix this later
class UserRepository {
  getAllUsers() {
    return db.users;
  }

  saveUser(user) {
    db.users.push(user);
  }
}`;

export function RunCommand() {
  return (
    <DocLayout>
      <h1>run</h1>
      <p className="doc-lead">
        Execute a tractor config file with multiple rules and operations. This is the recommended way to use tractor in a project.
      </p>

      <h2>Usage</h2>
      <CodeBlock code={`tractor run [CONFIG] [OPTIONS]`} language="bash" />
      <p>
        If <code>CONFIG</code> is omitted, tractor looks for <code>tractor.yml</code> in the current directory. Create one with <Link to="/docs/commands/init">tractor init</Link>. A different filename (e.g. <code>tractor.yaml</code>, <code>rules.toml</code>) works fine — just pass it as the argument.
      </p>

      <h2>Config File</h2>
      <p>
        A tractor config file defines rules, file patterns, and operations in YAML or TOML. Place it in your project root as <code>tractor.yml</code> and it will be picked up automatically.
      </p>

      <h3>Minimal Example</h3>
      <CodeBlock
        language="yaml"
        title="tractor.yml"
        code={`check:
  files:
    - "src/**/*.js"
  rules:
    - id: no-todo
      xpath: "//comment[contains(.,'TODO')]"
      reason: "TODO comments should be resolved"
      severity: warning`}
      />
      <CodeBlock language="bash" code={`tractor run`} />
      <OutputBlock output={`src/app.js:1:1: warning: TODO comments should be resolved
1 | // TODO: fix this later
    ^~~~~~~~~~~~~~~~~~~~~~~


1 warning in 1 file`} />

      <h3>Multiple Rules</h3>
      <Example
        file={{ name: 'example.js', language: 'js', content: EXAMPLE_JS }}
        command="tractor run"
        output={`app.js:1:1: warning: TODO comments should be resolved
1 | // TODO: fix this later
    ^~~~~~~~~~~~~~~~~~~~~~~

example.js:1:1: warning: TODO comments should be resolved
1 | // TODO: fix this later
    ^~~~~~~~~~~~~~~~~~~~~~~

example.js:3:3: error: getAll methods in repositories should use orderBy
3 |   getAllUsers() {
      ^~~~~~~~~~~


1 error in 2 files`}
      />
      <p>With this config:</p>
      <CodeBlock
        language="yaml"
        title="tractor.yml"
        code={`check:
  files:
    - "*.js"
  rules:
    - id: no-todo
      xpath: "//comment[contains(.,'TODO')]"
      reason: "TODO comments should be resolved"
      severity: warning

    - id: repository-needs-orderby
      xpath: >-
        //class[contains(name,'Repository')]
        //method[contains(name,'getAll')]
        [not(contains(.,'orderBy'))]/name
      reason: "getAll methods in repositories should use orderBy"
      severity: error`}
      />

      <h2>Rule Properties</h2>
      <table className="doc-table">
        <thead>
          <tr><th>Property</th><th>Required</th><th>Description</th></tr>
        </thead>
        <tbody>
          <tr><td><code>id</code></td><td>Yes</td><td>Unique identifier for the rule</td></tr>
          <tr><td><code>xpath</code></td><td>Yes</td><td><Link to="/docs/guides/query-syntax">Query</Link> — each match is a violation</td></tr>
          <tr><td><code>reason</code></td><td>Yes</td><td>Explanation shown for each violation</td></tr>
          <tr><td><code>severity</code></td><td>No</td><td><code>error</code> (default) or <code>warning</code></td></tr>
          <tr><td><code>message</code></td><td>No</td><td>Custom message template (<code>{'{value}'}</code>, <code>{'{line}'}</code>, etc.)</td></tr>
          <tr><td><code>include</code></td><td>No</td><td>File patterns for this rule only (relative to config file directory)</td></tr>
          <tr><td><code>exclude</code></td><td>No</td><td>File patterns to exclude for this rule (relative to config file directory)</td></tr>
          <tr><td><code>expect</code></td><td>No</td><td>Test examples (see below)</td></tr>
          <tr><td><code>variables</code></td><td>No</td><td>Rule-scoped values, available as <code>$rule.variables</code> in the query (see Variables)</td></tr>
        </tbody>
      </table>

      <h2>Rule Testing with expect</h2>
      <p>
        Add <code>expect</code> entries to validate your rules directly in the config:
      </p>
      <CodeBlock
        language="yaml"
        title="tractor.yml"
        code={`check:
  files:
    - "src/**/*.js"
  rules:
    - id: no-todo
      xpath: "//comment[contains(.,'TODO')]"
      reason: "TODO comments should be resolved"
      severity: error
      expect:
        - valid: "class Clean { }"
        - invalid: "// TODO: fix this"`}
      />
      <p>
        When you run <code>tractor run</code>, the <code>expect</code> entries are also validated. If a <code>valid</code> example matches the rule (or an <code>invalid</code> example doesn't), the run fails.
      </p>

      <h2>Variables</h2>
      <p>
        Declare values once and reference them from any query. Root-level <code>variables</code> are
        bound as the <code>$variables</code> map in every operation's XPath context. Each operation
        entry can also declare its own <code>variables</code>, bound under a namespace that mirrors
        the config key the entry lives under: <code>$rule.variables</code> for check rules,{' '}
        <code>$mapping.variables</code> for set mappings, <code>$query.variables</code> for query
        entries, and <code>$assertion.variables</code> for test assertions. All sit alongside the
        built-in <code>$file</code> (the current file path).
      </p>
      <CodeBlock
        language="yaml"
        title="tractor.yml"
        code={`variables:
  env: production
  banned: [eval, exec]

check:
  files:
    - "src/**/*.js"
  rules:
    - id: no-banned-calls
      xpath: "//call/name[. = $variables?banned?*]"
      reason: "banned function call"

    - id: not-too-many-params
      xpath: "//function[count(params/param) > $rule.variables?max]"
      reason: "too many parameters"
      variables:
        max: 4`}
      />
      <p>
        Values follow the JSON data model: strings, numbers, booleans, <code>null</code>, lists, and
        nested mappings. Scalars are read with map lookup (<code>$variables?env</code>), lists expand
        with <code>?*</code> (<code>$variables?banned?*</code>), and nested mappings chain lookups
        (<code>$variables?limits?max</code>). Numbers compare numerically; a lookup on a key that
        isn't configured yields the empty sequence, so predicates simply don't match. Because a
        typo'd key would silently disable a rule, every operation emits an advisory warning (never
        a failure) for literal lookups on keys the config doesn't define — and for references to an
        entry namespace that isn't bound in the current operation (e.g.{' '}
        <code>$rule.variables</code> inside a set mapping, which is an empty map there).
      </p>
      <p>
        The same pattern works in every operation. A set mapping can select its targets through its
        own values, and a test assertion can parameterize its threshold:
      </p>
      <CodeBlock
        language="yaml"
        title="tractor.yml"
        code={`set:
  files: ["config/*.json"]
  mappings:
    - xpath: "//port[. = $mapping.variables?from]"
      value: "3000"
      variables:
        from: 8080

test:
  files: ["src/**/*.js"]
  assertions:
    - xpath: "//function[count(params/param) > $assertion.variables?max]"
      expect: none
      variables:
        max: 4`}
      />
      <h3>Variable sources</h3>
      <p>
        A value in any <code>variables</code> tree can come from an external source instead of
        being written inline: a map with a single <code>$</code>-prefixed key names the source.
        Each source binds under its own key — sources never merge, so two sources cannot collide.
        Directives may appear at any depth, so one nested key can be file-sourced while its
        siblings stay inline.
      </p>
      <CodeBlock
        language="yaml"
        title="tractor.yml"
        code={`variables:
  env: production            # inline literal
  settings:
    $file: "config/vars.yml" # whole file bound under this name
  team:
    prefixes:
      $file: "prefixes.json" # nested directive; siblings stay inline`}
      />
      <p>
        <code>$file</code> loads a JSON, YAML, or TOML document (path relative to the config
        file's directory); read it with the usual lookups, e.g.{' '}
        <code>$variables?settings?db?host</code>. File content is pure data — directives inside
        loaded files are not processed. <code>$literal</code> is the escape hatch for literal data
        whose keys start with <code>$</code>: it keeps its content verbatim.
      </p>
      <p>
        Directives are validated when the config loads — an unknown directive (e.g. a typo like{' '}
        <code>$fiel</code>) fails before anything runs, with no file access. The file{' '}
        <em>read</em> happens later, at the start of each operation, so a file written by an{' '}
        <em>earlier</em> operation in the same run is read fresh by the next one; every rule
        within one operation sees the same snapshot.
      </p>
      <p>
        A source is only read where it is actually used. That is decided per entry: a rule reading{' '}
        <code>$rule.variables</code> does not force a sibling rule's sources to load. So a source
        that nothing reads never fails the run — which is what lets a config declare a file that a
        later operation is about to write. A source that <em>is</em> read and whose file is missing
        fails the run at that operation.
      </p>
      <p>
        Rules also get <code>$rule.id</code> — the current rule's <code>id</code> string. This makes
        per-rule escape hatches a single shared pattern instead of hand-written per rule:
      </p>
      <CodeBlock
        language="yaml"
        title="tractor.yml"
        code={`# a comment \`tractor:allow(<rule-id>)\` in the enclosing function
# suppresses exactly that rule
xpath: >-
  //call//object[.='console']
  [not(ancestor::function[.//comment[
    contains(., concat('tractor:allow(', $rule.id, ')'))]])]`}
      />

      <h3>Materialized query results (multi-query rules)</h3>
      <p>
        A query operation can write its results to a JSON index with <code>output</code>, and a
        later operation can consume that file as a variable — cross-file assertions in a single
        run. Because sources resolve at each operation's start, the check below reads the file
        the query just wrote:
      </p>
      <CodeBlock
        language="yaml"
        title="tractor.yml"
        code={`variables:
  repos:
    $file: "gathered/repos.json"

operations:
  - query:
      files: ["src/**/*.cs"]
      queries:
        - xpath: "//class/name[contains(., 'Repository')]"
      output: "gathered/repos.json"

  - check:
      files: ["src/**/*.cs"]
      rules:
        - id: entity-needs-repository
          xpath: >-
            //class/name[not(contains(., 'Repository'))]
            [not(concat(., 'Repository') = $variables?repos?files?*?*)]
          reason: "entity class has no matching repository"`}
      />
      <p>
        The written file is an index keyed by source file:{' '}
        <code>{'{ "files": { "<path>": [ ... ] } }'}</code>, with paths relative to the config
        directory so the artifact is stable and committable. Each entry is a labelled object;{' '}
        <code>view:</code> chooses which keys it holds (<code>value</code> by default, plus{' '}
        <code>line</code>, <code>column</code>, <code>tree</code>) and nothing else — the shape
        never depends on how many fields you selected or on what a match contains, so adding a
        field only adds a key. Updates merge incrementally: entries for every queried file are
        replaced wholesale, files outside the queried set (e.g. excluded by{' '}
        <code>diff-files</code>) keep their entries, and entries whose file no longer exists are
        pruned — so a diff-scoped gather stays correct without re-querying the whole tree.
      </p>
      <p>
        <strong>Reading the index.</strong> Most rules just want the flat set of gathered values.
        That is <code>?files?*?*?value</code> — the first <code>?*</code> expands every file key,
        the second every entry in that file's array, and <code>?value</code> takes the field:
      </p>
      <CodeBlock
        language="text"
        code={`$variables?records?files?*?*?value              all values, provenance discarded
$variables?records?files?("src/A.cs")?*?value  values from one specific file
$variables?records?files?*?*?tree              the structured record of each entry
map:keys($variables?records?files)             the file paths themselves`}
      />
      <p>
        Keep the file keys when a rule is <em>about</em> paths (e.g. "every file with an X must
        have a matching Y"); use the flat form for plain membership tests.
      </p>

      <h3>Correspondence rules</h3>
      <p>
        The common shape is "this thing over here must agree with that thing over there" — a
        DTO's <code>[MaxLength]</code> matching its Record's, say. Gather one side as{' '}
        <strong>structured records</strong> with a <code>map{'{}'}</code> constructor and{' '}
        <code>view: [tree]</code>, then compare field by field from the other side:
      </p>
      <CodeBlock
        language="yaml"
        title="tractor.yml"
        code={`variables:
  records:
    $file: "gathered/records.json"

operations:
  - query:
      files: ["**/*Record.cs"]
      queries:
        - xpath: >-
            //property[.//attribute/name/ref = 'MaxLength']
            ! map { 'name': string(name), 'max': string(.//argument/int) }
      output:
        file: "gathered/records.json"
        view: [tree]          # each entry holds its map under "tree"

  - check:
      files: ["**/*Dto.cs"]
      rules:
        - id: dto-maxlength-drift
          reason: "DTO MaxLength differs from the Record"
          xpath: >-
            //property[.//attribute/name/ref = 'MaxLength']
            [not(some $r in $variables?records?files?*?*?tree
                 satisfies $r?name = string(name) and $r?max = string(.//argument/int))]
            /name`}
      />
      <p>
        Each field is compared on its own (<code>$r?name</code>, <code>$r?max</code>), so the
        record stays self-describing and adding a third field doesn't change how the existing
        ones match.
      </p>
      <p>
        A single scalar per match — <code>concat(name, ':', .//argument/int)</code> with the
        default <code>view: [value]</code> — is shorter, and fine for a plain membership test
        against one value. Prefer the structured form for anything with more than one field:
        packing fields into a string makes the parts positional and invisible to the reader, and
        the separator becomes load-bearing (with no separator, <code>a</code>+<code>bc</code>{' '}
        collides with <code>ab</code>+<code>c</code>). If the two sides name things differently,
        normalize inside the field expression (<code>replace()</code>,{' '}
        <code>substring-before()</code>) rather than after.
      </p>
      <p>
        <strong>Known limitation:</strong> matching is per file and syntactic, so an inherited
        member is invisible to the side that inherits it — a property declared on a shared base
        class in another file will look "missing" to a rule that only inspects the derived
        declaration. Gathering the base declarations into the same index (a second{' '}
        <code>query</code> writing to the same <code>output:</code> file, or a wider{' '}
        <code>files:</code> pattern) covers the common cases; full type resolution across files
        is out of scope.
      </p>

      <h2>Multiple Operation Types</h2>
      <p>
        Use the <code>operations</code> list to mix check, test, query, and set operations:
      </p>
      <CodeBlock
        language="yaml"
        title="tractor.yml"
        code={`files:
  - "src/**/*.js"

operations:
  - check:
      rules:
        - id: no-todo
          xpath: "//comment[contains(.,'TODO')]"
          reason: "TODO comments should be resolved"
          severity: warning

  - test:
      assertions:
        - xpath: "//class"
          expect: some
          message: "At least one class expected"`}
      />

      <h3>Execution order</h3>
      <p>
        Operations run one after another, and each sees whatever the previous ones left behind
        — files a <code>set</code> rewrote, an index a <code>query</code> materialized. Two ways
        to arrange them:
      </p>
      <ul>
        <li>
          The <code>operations</code> list runs <strong>exactly in the order written</strong>.
          This is how you express a dependency, and the only way to run two operations of the
          same kind.
        </li>
        <li>
          The root-level shorthand keys (<code>query</code>, <code>check</code>, <code>set</code>,{' '}
          <code>test</code>) are a convenience for at most one operation of each kind. They run
          in that fixed order — <strong>query first</strong>, so a gathered index is available to
          the operations that consume it.
        </li>
      </ul>
      <p>
        <strong>Changed behaviour:</strong> the shorthand order was previously check, set, query.
        A config that used root-level <code>set</code> and <code>query</code> together therefore
        had the query observe files <em>after</em> the set rewrote them, and now observes them
        before. If that ordering mattered, write the two as an explicit{' '}
        <code>operations</code> list, which says what you mean and is unaffected.
      </p>

      <h2>Set Operations</h2>
      <p>
        Use <code>set</code> to apply multiple value changes in a config file. Each mapping specifies an expression and the value to set. This is the batch equivalent of the <Link to="/docs/commands/set">set command</Link>:
      </p>
      <CodeBlock
        language="yaml"
        title="tractor.yml"
        code={`set:
  files: ["app-config.json"]
  mappings:
    - xpath: "//database/host"
      value: "db.prod.internal"
    - xpath: "//database/port"
      value: "5432"
    - xpath: "//cache/ttl"
      value: "600"`}
      />
      <p>
        All mappings apply to the matched files in a single operation. This is the recommended way to set multiple values at once — instead of running <code>tractor set</code> repeatedly for each value.
      </p>
      <p>
        Set operations can also be mixed with other operation types using the <code>operations</code> list:
      </p>
      <CodeBlock
        language="yaml"
        title="tractor.yml"
        code={`operations:
  - check:
      files: ["settings.yaml"]
      rules:
        - id: no-debug
          xpath: "//debug[.='true']"
          reason: "debug should be disabled"
  - set:
      files: ["app-config.json"]
      mappings:
        - xpath: "//database/host"
          value: "db.prod.internal"
        - xpath: "//cache/ttl"
          value: "600"`}
      />

      <h2>Scope and File Resolution</h2>
      <p>
        File patterns can be set at multiple levels. Each level narrows the scope — it never widens it. The effective file set is the intersection of all levels that are defined.
      </p>
      <p>
        All file patterns — root <code>files</code>, operation <code>files</code>, <code>exclude</code>, and per-rule <code>include</code>/<code>exclude</code> — are
        resolved relative to the config file's directory. Absolute paths (e.g. from an IDE) are used as-is.
      </p>

      <h3>Intersection chain</h3>
      <p>
        When you run <code>tractor run --config config.yaml frontend/**/*.js</code>, the file resolution works like this:
      </p>
      <CodeBlock
        language="text"
        code={`config root files  ∩  operation files  ∩  CLI files
     (broadest)         (per-operation)      (narrowest)`}
      />
      <ul>
        <li>If a level is <strong>not defined</strong>, it's skipped entirely (no intersection).</li>
        <li>If a level is defined but its <strong>patterns match nothing</strong>, the result is empty.</li>
      </ul>

      <h3>Root and operation files</h3>
      <CodeBlock
        language="yaml"
        code={`# Root-level: defines the broadest scope for all operations
files:
  - "src/**/*.js"
exclude:
  - "src/generated/**"

operations:
  - check:
      # This operation's files intersect with root files.
      # Effective: src/core/**/*.js that are also in src/**/*.js
      files:
        - "src/core/**/*.js"
      rules:
        - id: no-todo
          xpath: "//comment[contains(.,'TODO')]"
          reason: "No TODOs in core"

  - check:
      # No files specified — uses root files as the base.
      # Effective: src/**/*.js
      rules:
        - id: no-eval
          xpath: "//call[name='eval']"
          reason: "eval is not allowed"`}
      />
      <ul>
        <li><strong>files</strong>: Operation files intersect with root files — only files matching both patterns are processed.</li>
        <li><strong>No files on operation</strong>: Root files are used as the base.</li>
        <li><strong>No files anywhere</strong>: If neither root nor operation specifies files, CLI file arguments are used as the base set.</li>
        <li><strong>Missing vs empty</strong>: Omitting <code>files:</code> entirely means "unrestricted" (no intersection at that level). Writing <code>files: []</code> explicitly means "no files" — the result will be empty.</li>
        <li><strong>exclude</strong>: Union of root and operation excludes (both narrow the scope).</li>
      </ul>

      <h3>CLI file arguments</h3>
      <p>
        Pass files or globs as positional arguments to narrow the config's scope to specific files:
      </p>
      <CodeBlock language="bash" code={`# Run config rules, but only on these files
tractor run src/app.js src/utils.js

# Or with globs
tractor run "src/core/**/*.js"

# Absolute paths work too (e.g. from an IDE)
tractor run /home/user/project/src/app.js

# Use a non-default config with --config
tractor run --config rules.yaml src/app.js`} />
      <p>
        CLI files are intersected with the resolved config scope. This is useful for checking only the files you changed, without modifying the config.
        If the config has no <code>files:</code> key (neither root nor operation level), CLI files are used directly as the file set.
      </p>

      <h3>Per-rule include/exclude</h3>
      <p>
        Individual rules can further narrow their scope with <code>include</code> and <code>exclude</code> patterns.
        These are resolved relative to the config file's directory — not the current working directory:
      </p>
      <CodeBlock
        language="yaml"
        code={`check:
  files: ["src/**/*.js"]
  rules:
    - id: no-todo
      xpath: "//comment[contains(.,'TODO')]"
      reason: "No TODOs in production code"
      exclude: ["src/test/**"]    # skip test files

    - id: no-console-log
      xpath: "//call[name='console.log']"
      reason: "Use the logger instead"
      include: ["src/core/**"]    # only check core files`}
      />
      <p>
        Per-rule <code>include</code> narrows which files the rule applies to. Per-rule <code>exclude</code> removes files from consideration.
        These are applied after the operation-level file resolution, so they can only narrow — never widen — the file set.
      </p>

      <h3>File limits</h3>
      <p>
        Tractor protects against accidentally globbing too many files:
      </p>
      <ul>
        <li><code>--max-files</code> (default: 10,000) — maximum files to process.</li>
        <li>Each glob pattern aborts if it expands past 10× the max-files limit.</li>
        <li>If all patterns match 0 files, tractor reports a fatal error instead of silently succeeding.</li>
      </ul>
      <CodeBlock language="bash" code={`# Increase the limit for large monorepos
tractor run --max-files 50000`} />

      <h3>Debugging with --verbose</h3>
      <p>
        Use <code>--verbose</code> to see each file resolution step — which patterns are being expanded, the base directory, and how many files remain after each intersection:
      </p>
      <CodeBlock
        language="text"
        code={`$ tractor run --verbose
  files: resolving relative to /home/user/project
  files: max 10000 files, expansion limit 100000
  files: expanding root scope "src/**/*.js" ...
  files: root scope has 342 file(s)
  files: expanding operation "src/core/**/*.js" ...
  files: operation has 48 file(s)
  files: 48 file(s) after root intersection (was 48)`}
      />

      <h2>Git-aware Filtering</h2>
      <p>
        Only check files or lines changed in a git diff:
      </p>
      <CodeBlock
        language="yaml"
        code={`# Only check files changed vs main branch
diff-files: "main..HEAD"

check:
  files:
    - "src/**/*.js"
  rules:
    - id: no-todo
      xpath: "//comment[contains(.,'TODO')]"
      reason: "No TODOs"`}
      />
      <p>
        You can also use <code>diff-lines</code> to restrict matches to changed hunks only, and override from the CLI with <code>--diff-files</code> or <code>--diff-lines</code>.
      </p>

      <h2>Options Reference</h2>
      <table className="doc-table">
        <thead>
          <tr><th>Option</th><th>Description</th></tr>
        </thead>
        <tbody>
          <tr><td><code>-f, --format</code></td><td>Output format: gcc (default), github, text, json, yaml, xml, claude-code</td></tr>
          <tr><td><code>-v, --view</code></td><td>View fields to include</td></tr>
          <tr><td><code>-m, --message</code></td><td>Message template for matches</td></tr>
          <tr><td><code>--diff-files</code></td><td>Only files changed in a git diff range</td></tr>
          <tr><td><code>--diff-lines</code></td><td>Only matches in changed hunks</td></tr>
          <tr><td><code>--max-files</code></td><td>Maximum files to process (default: 10,000)</td></tr>
          <tr><td><code>--verbose</code></td><td>Show file resolution steps and diagnostics on stderr</td></tr>
          <tr><td><code>-c, --concurrency</code></td><td>Number of parallel workers</td></tr>
        </tbody>
      </table>

      <div className="doc-next">
        <p>Next: <Link to="/docs/guides/writing-queries">Writing Queries guide</Link> — learn to write tractor queries step by step.</p>
      </div>
    </DocLayout>
  );
}
