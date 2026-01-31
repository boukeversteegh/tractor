#!/usr/bin/env node
/**
 * Generate grammar-versions.json from installed packages and source builds
 */

import fs from 'fs';
import path from 'path';
import { execSync } from 'child_process';
import { fileURLToPath } from 'url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const ROOT = path.join(__dirname, '..');
const OUTPUT = path.join(ROOT, 'public/versions.json');
const BUILD_DIR = path.join(ROOT, '.grammar-build');

// Source builds (not from npm) - add languages here that are built from git
const SOURCE_BUILDS = {
  'csharp': { repo: 'tree-sitter/tree-sitter-c-sharp', ref: 'master', dir: 'tree-sitter-c-sharp' },
};

// Language to package mapping (for languages where names differ)
const LANG_PACKAGE_MAP = {
  'tsx': 'tree-sitter-typescript',
};

function getPackageVersion(packageName) {
  try {
    const pkgPath = path.join(ROOT, 'node_modules', packageName, 'package.json');
    const pkg = JSON.parse(fs.readFileSync(pkgPath, 'utf8'));
    return pkg.version;
  } catch {
    return null;
  }
}

function getGitInfo(repoDir) {
  try {
    const commit = execSync('git rev-parse --short HEAD', { cwd: repoDir, encoding: 'utf8' }).trim();
    const date = execSync('git log -1 --format=%cs', { cwd: repoDir, encoding: 'utf8' }).trim();
    return { commit, date };
  } catch {
    return null;
  }
}

function main() {
  const versions = {};

  // Find all grammar TS files to know which languages we have
  const grammarsDir = path.join(ROOT, 'src/grammars');

  if (!fs.existsSync(grammarsDir)) {
    console.log('No grammars directory found, skipping version generation');
    return;
  }

  const grammarFiles = fs.readdirSync(grammarsDir)
    .filter(f => f.endsWith('.ts') && f !== 'index.ts')
    .map(f => f.replace('.ts', ''));

  for (const lang of grammarFiles) {
    // Check if it's a source build
    if (SOURCE_BUILDS[lang]) {
      const { repo, ref, dir } = SOURCE_BUILDS[lang];
      const repoDir = path.join(BUILD_DIR, dir);
      const gitInfo = getGitInfo(repoDir);

      if (gitInfo) {
        versions[lang] = {
          source: 'git',
          repo,
          ref,
          commit: gitInfo.commit,
          date: gitInfo.date,
        };
      } else {
        // Fallback if git info not available
        versions[lang] = { source: 'git', repo, ref, commit: 'unknown', date: 'unknown' };
      }
    } else {
      // npm package
      const packageName = LANG_PACKAGE_MAP[lang] || `tree-sitter-${lang}`;
      const version = getPackageVersion(packageName);

      if (version) {
        versions[lang] = {
          source: 'npm',
          package: packageName,
          version,
        };
      }
    }
  }

  // Add runtime version
  const runtimeVersion = getPackageVersion('web-tree-sitter');
  if (runtimeVersion) {
    versions._runtime = {
      package: 'web-tree-sitter',
      version: runtimeVersion,
    };
  }

  // Add generation timestamp
  versions._generated = new Date().toISOString();

  // Write output
  fs.writeFileSync(OUTPUT, JSON.stringify(versions, null, 2) + '\n');
  console.log(`Generated ${OUTPUT}`);
  console.log(`  ${Object.keys(versions).filter(k => !k.startsWith('_')).length} languages`);
}

main();
