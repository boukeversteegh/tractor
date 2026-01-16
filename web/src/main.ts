/**
 * Tractor Web - Main entry point
 */

import { initParser, parseSource } from './parser';
import { initTractor, parseAstToXmlSimple } from './tractor';
import { queryXml, Match } from './xpath';

// DOM elements
let sourceInput: HTMLTextAreaElement;
let languageSelect: HTMLSelectElement;
let rawModeCheckbox: HTMLInputElement;
let showLocationsCheckbox: HTMLInputElement;
let prettyPrintCheckbox: HTMLInputElement;
let xmlOutput: HTMLElement;
let xpathInput: HTMLInputElement;
let queryResults: HTMLElement;
let loadingOverlay: HTMLElement;
let copyButton: HTMLButtonElement;

// Current state
let currentXml = '';

// Sample code snippets for each language
const SAMPLE_CODE: Record<string, string> = {
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
  scala: `def hello(name: String): String = {
  s"Hello, $name!"
}`,
  lua: `function hello(name)
  return "Hello, " .. name .. "!"
end`,
  haskell: `hello :: String -> String
hello name = "Hello, " ++ name ++ "!"`,
  ocaml: `let hello name =
  "Hello, " ^ name ^ "!"`,
  r: `hello <- function(name) {
  paste("Hello,", name, "!")
}`,
  julia: `function hello(name)
    "Hello, $(name)!"
end`,
};

/**
 * Initialize the application
 */
async function init(): Promise<void> {
  // Get DOM elements
  sourceInput = document.getElementById('source-input') as HTMLTextAreaElement;
  languageSelect = document.getElementById('language-select') as HTMLSelectElement;
  rawModeCheckbox = document.getElementById('raw-mode') as HTMLInputElement;
  showLocationsCheckbox = document.getElementById('show-locations') as HTMLInputElement;
  prettyPrintCheckbox = document.getElementById('pretty-print') as HTMLInputElement;
  xmlOutput = document.querySelector('#xml-output code') as HTMLElement;
  xpathInput = document.getElementById('xpath-input') as HTMLInputElement;
  queryResults = document.getElementById('query-results') as HTMLElement;
  loadingOverlay = document.getElementById('loading') as HTMLElement;
  copyButton = document.getElementById('copy-xml') as HTMLButtonElement;

  try {
    // Initialize parsers
    await Promise.all([initParser(), initTractor()]);

    // Hide loading overlay
    loadingOverlay.classList.add('hidden');

    // Set up event listeners
    setupEventListeners();

    // Initial parse
    await updateOutput();
  } catch (error) {
    console.error('Failed to initialize:', error);
    loadingOverlay.innerHTML = `
      <div class="error">
        <h2>Failed to initialize</h2>
        <p>${error instanceof Error ? error.message : 'Unknown error'}</p>
        <p>Make sure the WASM modules are built and grammar files are available.</p>
      </div>
    `;
  }
}

/**
 * Set up event listeners
 */
function setupEventListeners(): void {
  // Debounced source input handler
  let sourceTimeout: ReturnType<typeof setTimeout>;
  sourceInput.addEventListener('input', () => {
    clearTimeout(sourceTimeout);
    sourceTimeout = setTimeout(updateOutput, 300);
  });

  // Language change handler - update sample code and output
  languageSelect.addEventListener('change', () => {
    const lang = languageSelect.value;
    if (SAMPLE_CODE[lang]) {
      sourceInput.value = SAMPLE_CODE[lang];
    }
    updateOutput();
  });

  // Raw mode change handler
  rawModeCheckbox.addEventListener('change', updateOutput);

  // Show locations change handler
  showLocationsCheckbox.addEventListener('change', updateOutput);

  // Pretty print change handler
  prettyPrintCheckbox.addEventListener('change', updateOutput);

  // XPath input handler
  let xpathTimeout: ReturnType<typeof setTimeout>;
  xpathInput.addEventListener('input', () => {
    clearTimeout(xpathTimeout);
    xpathTimeout = setTimeout(updateQueryResults, 300);
  });

  // Copy button handler
  copyButton.addEventListener('click', copyXmlToClipboard);
}

/**
 * Update the XML output
 */
async function updateOutput(): Promise<void> {
  const source = sourceInput.value;
  const language = languageSelect.value;
  const rawMode = rawModeCheckbox.checked;
  const includeLocations = showLocationsCheckbox.checked;
  const prettyPrint = prettyPrintCheckbox.checked;

  if (!source.trim()) {
    xmlOutput.textContent = '';
    currentXml = '';
    updateQueryResults();
    return;
  }

  try {
    // Parse source to AST
    const ast = await parseSource(source, language);

    // Convert AST to XML
    currentXml = await parseAstToXmlSimple(ast, source, language, rawMode, includeLocations, prettyPrint);

    // Display XML with syntax highlighting
    xmlOutput.innerHTML = highlightXml(currentXml);

    // Update query results
    updateQueryResults();
  } catch (error) {
    console.error('Parse error:', error);
    xmlOutput.innerHTML = `<span class="error">Error: ${error instanceof Error ? error.message : 'Unknown error'}</span>`;
    currentXml = '';
    updateQueryResults();
  }
}

/**
 * Update XPath query results
 */
function updateQueryResults(): void {
  const xpath = xpathInput.value.trim();

  if (!xpath || !currentXml) {
    queryResults.innerHTML = '<span class="info">Enter an XPath query to search the XML</span>';
    return;
  }

  try {
    const matches = queryXml(currentXml, xpath);

    if (matches.length === 0) {
      queryResults.innerHTML = '<span class="info">No matches found</span>';
      return;
    }

    queryResults.innerHTML = matches
      .slice(0, 50) // Limit to 50 results
      .map((match) => formatMatch(match))
      .join('');

    if (matches.length > 50) {
      queryResults.innerHTML += `<div class="info">...and ${matches.length - 50} more matches</div>`;
    }
  } catch (error) {
    console.error('XPath error:', error);
    queryResults.innerHTML = `<span class="error">XPath error: ${error instanceof Error ? error.message : 'Unknown error'}</span>`;
  }
}

/**
 * Format a match for display
 */
function formatMatch(match: Match): string {
  const location = match.start ? `${match.start} - ${match.end}` : '';
  const xml = escapeHtml(match.xml.substring(0, 200));

  return `
    <div class="match">
      ${location ? `<div class="match-location">${location}</div>` : ''}
      <code>${xml}${match.xml.length > 200 ? '...' : ''}</code>
    </div>
  `;
}

/**
 * Simple XML syntax highlighting
 */
function highlightXml(xml: string): string {
  // Use placeholders to avoid regex matching inserted spans
  const escaped = escapeHtml(xml);

  // Build result by processing character by character to avoid conflicts
  let result = '';
  let i = 0;

  while (i < escaped.length) {
    // Check for tag start: &lt;
    if (escaped.slice(i, i + 4) === '&lt;') {
      result += '&lt;';
      i += 4;

      // Capture tag name (including optional /)
      let tagName = '';
      if (escaped[i] === '/') {
        tagName += '/';
        i++;
      }
      while (i < escaped.length && /[\w-]/.test(escaped[i])) {
        tagName += escaped[i];
        i++;
      }
      if (tagName) {
        result += `<span class="xml-tag">${tagName}</span>`;
      }
    }
    // Check for attribute: space + name + =
    else if (/\s/.test(escaped[i])) {
      result += escaped[i];
      i++;

      // Look for attribute name
      let attrName = '';
      while (i < escaped.length && /[\w-]/.test(escaped[i])) {
        attrName += escaped[i];
        i++;
      }
      if (attrName && escaped[i] === '=') {
        result += `<span class="xml-attr">${attrName}</span>=`;
        i++; // skip =

        // Look for quoted value
        if (escaped[i] === '"') {
          result += '"';
          i++;
          let attrValue = '';
          while (i < escaped.length && escaped[i] !== '"') {
            attrValue += escaped[i];
            i++;
          }
          result += `<span class="xml-value">${attrValue}</span>`;
          if (escaped[i] === '"') {
            result += '"';
            i++;
          }
        }
      } else {
        result += attrName;
      }
    }
    else {
      result += escaped[i];
      i++;
    }
  }

  return result;
}

/**
 * Escape HTML entities
 */
function escapeHtml(text: string): string {
  const div = document.createElement('div');
  div.textContent = text;
  return div.innerHTML;
}

/**
 * Copy XML to clipboard
 */
async function copyXmlToClipboard(): Promise<void> {
  if (!currentXml) return;

  try {
    await navigator.clipboard.writeText(currentXml);
    copyButton.textContent = 'Copied!';
    setTimeout(() => {
      copyButton.textContent = 'Copy';
    }, 2000);
  } catch (error) {
    console.error('Failed to copy:', error);
  }
}

// Start the application
document.addEventListener('DOMContentLoaded', init);
