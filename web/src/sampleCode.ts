// Sample code snippets for each language
export const SAMPLE_CODE: Record<string, string> = {
  typescript: `function hello(name: string): string {
  return \`Hello, \${name}!\`;
}`,
  javascript: `function hello(name) {
  return \`Hello, \${name}!\`;
}`,
  csharp: `public class Greeter {
    public string Hello(string name) {
        return $"Hello, {name}!";
    }
}`,
  rust: `fn hello(name: &str) -> String {
    format!("Hello, {}!", name)
}`,
  python: `def hello(name: str) -> str:
    return f"Hello, {name}!"`,
  go: `func hello(name string) string {
    return fmt.Sprintf("Hello, %s!", name)
}`,
  java: `public class Greeter {
    public String hello(String name) {
        return "Hello, " + name + "!";
    }
}`,
  ruby: `def hello(name)
  "Hello, #{name}!"
end`,
  cpp: `std::string hello(const std::string& name) {
    return "Hello, " + name + "!";
}`,
  c: `char* hello(const char* name) {
    char* result = malloc(256);
    sprintf(result, "Hello, %s!", name);
    return result;
}`,
  json: `{
  "greeting": "Hello",
  "name": "World"
}`,
  html: `<!DOCTYPE html>
<html>
  <body>
    <h1>Hello, World!</h1>
  </body>
</html>`,
  css: `.greeting {
  color: blue;
  font-size: 24px;
}`,
  bash: `#!/bin/bash
hello() {
  echo "Hello, $1!"
}`,
  yaml: `greeting:
  message: Hello
  name: World`,
  php: `<?php
function hello($name) {
    return "Hello, $name!";
}`,
};
