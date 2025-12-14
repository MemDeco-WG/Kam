#!/usr/bin/env node
/**
 * i18n coverage script for the Rust `src` i18n TOML files.
 *
 * - Scans `Kam/src/i18n/{en,zh}.toml` by default (configurable via CLI).
 * - Parses simple TOML key = "string" assignments (also supports simple string arrays).
 * - Computes coverage (how many keys in `from` have non-empty values in `to`),
 *   and how many are actually different from the source (likely translated).
 *
 * Usage:
 *   node Kam/scripts/i18n-coverage-src.js [--from=en] [--to=zh] [--dir=path/to/i18n] [--fail-under=80] [--format=json] [--max-missing=50]
 *
 * Examples:
 *   node Kam/scripts/i18n-coverage-src.js
 *   node Kam/scripts/i18n-coverage-src.js --from=en --to=zh --fail-under=90
 *   node Kam/scripts/i18n-coverage-src.js --format=json --max-missing=100
 *
 * Exit codes:
 *   0: OK
 *   1: Error (e.g., missing files, parse error)
 *   2: Coverage check failed due to --fail-under threshold
 */

const fs = require('fs');
const path = require('path');

function parseArgs(argv) {
  const args = {};
  argv.forEach(a => {
    if (!a.startsWith('--')) return;
    const eq = a.indexOf('=');
    if (eq === -1) args[a.slice(2)] = true;
    else args[a.slice(2, eq)] = a.slice(eq + 1);
  });
  return args;
}

function usageAndExit(code = 0) {
  console.log('Usage: node scripts/i18n-coverage-src.js [--from=en] [--to=zh] [--dir=path/to/i18n] [--fail-under=80] [--format=json] [--max-missing=50]');
  process.exit(code);
}

/**
 * Parse a TOML-style string file and extract dotted keys mapped to string or array-of-string values.
 *
 * Supported:
 *   - dotted keys:   key.subkey = "value"
 *   - table headers: [table] followed by `key = "value"` => table.key = value
 *   - simple arrays: arr = ["a", "b", "c"]  (only basic same-line arrays supported)
 *
 * Limitations:
 *   - Does not implement full TOML spec (no multiline basic strings, no inline tables, no nested inline arrays).
 *   - Designed to work with our repository's consistent style.
 */
function parseTomlStrings(content) {
  const lines = content.split(/\r?\n/);
  const map = {};
  let currentTable = '';

  function qualifyKey(k) {
    k = k.trim();
    // Remove surrounding quotes on key if present (rare)
    if ((k.startsWith('"') && k.endsWith('"')) || (k.startsWith("'") && k.endsWith("'"))) {
      k = k.slice(1, -1);
    }
    if (!k.includes('.') && currentTable) return `${currentTable}.${k}`;
    return k;
  }

  for (let raw of lines) {
    let line = raw.trim();
    if (!line) continue;
    if (line.startsWith('#')) continue;

    // Table header: [table.name]
    if (line.startsWith('[')) {
      const m = line.match(/^\[([^\]]+)\]\s*$/);
      if (m) {
        currentTable = m[1].trim();
      }
      continue;
    }

    const eq = line.indexOf('=');
    if (eq === -1) continue; // not a key assignment

    let keyPart = line.slice(0, eq).trim();
    let valPart = line.slice(eq + 1).trim();

    if (!keyPart) continue;

    // If valPart starts with a quote, parse a quoted string (supports escaped quotes)
    if (valPart[0] === '"' || valPart[0] === "'") {
      const quote = valPart[0];
      let i = 1;
      let escaped = false;
      for (; i < valPart.length; i++) {
        const ch = valPart[i];
        if (escaped) { escaped = false; continue; }
        if (ch === '\\') { escaped = true; continue; }
        if (ch === quote) break;
      }
      if (i >= valPart.length) {
        // Malformed line; skip
        continue;
      }
      const quoted = valPart.slice(0, i + 1);
      let parsed;
      try {
        if (quote === '"') {
          // JSON.parse is safe for double-quoted strings to unescape sequences
          parsed = JSON.parse(quoted);
        } else {
          // single-quoted TOML strings are literal: take inner content as-is (no escapes)
          parsed = quoted.slice(1, -1);
        }
      } catch (e) {
        // Fallback: raw inner
        parsed = quoted.slice(1, -1);
      }
      map[qualifyKey(keyPart)] = parsed;
      continue;
    }

    // If valPart starts with '[' attempt to parse a simple inline array of strings
    if (valPart[0] === '[') {
      // find closing bracket
      let i = 1;
      let depth = 1;
      let inQuote = false;
      let qch = '';
      let prev = '';
      for (; i < valPart.length; i++) {
        const ch = valPart[i];
        if (inQuote) {
          if (ch === qch && prev !== '\\') {
            inQuote = false;
            qch = '';
          }
        } else {
          if ((ch === '"' || ch === "'")) {
            inQuote = true;
            qch = ch;
          } else if (ch === '[') {
            depth++;
          } else if (ch === ']') {
            depth--;
            if (depth === 0) break;
          }
        }
        prev = ch;
      }
      if (i >= valPart.length) {
        // malformed
        continue;
      }
      const inner = valPart.slice(1, i).trim();
      // split by commas not inside quotes
      const items = [];
      let cur = '';
      inQuote = false;
      qch = '';
      prev = '';
      for (let j = 0; j < inner.length; j++) {
        const ch = inner[j];
        if (inQuote) {
          cur += ch;
          if (ch === qch && prev !== '\\') {
            inQuote = false;
            qch = '';
          }
        } else {
          if (ch === '"' || ch === "'") {
            inQuote = true;
            qch = ch;
            cur += ch;
          } else if (ch === ',') {
            items.push(cur.trim());
            cur = '';
          } else {
            cur += ch;
          }
        }
        prev = ch;
      }
      if (cur.trim()) items.push(cur.trim());
      // parse items into strings where possible
      const parsedItems = items.map(it => {
        if (!it) return '';
        if ((it.startsWith('"') && it.endsWith('"')) || (it.startsWith("'") && it.endsWith("'"))) {
          try {
            if (it[0] === '"') return JSON.parse(it);
            else return it.slice(1, -1);
          } catch (e) {
            return it.slice(1, -1);
          }
        } else {
          return it;
        }
      });
      map[qualifyKey(keyPart)] = parsedItems;
      continue;
    }

    // Not a string or array we recognize; skip (numbers, booleans are not translatable)
  }

  return map;
}

function flattenMapToLeaves(map) {
  // Converts map { key: string | [strings] } -> array of { path, value }
  const out = [];
  Object.keys(map).sort().forEach(k => {
    const v = map[k];
    if (typeof v === 'string') out.push({ path: k, value: v });
    else if (Array.isArray(v)) {
      v.forEach((el, idx) => {
        if (typeof el === 'string') out.push({ path: `${k}[${idx}]`, value: el });
      });
    }
    // ignore other types
  });
  return out;
}

function pct(n, total) {
  if (total === 0) return '100.0%';
  return ( (n / total) * 100 ).toFixed(1) + '%';
}

function analyze(fromMap, toMap) {
  const fromLeaves = flattenMapToLeaves(fromMap);
  const total = fromLeaves.length;
  let present = 0;
  let different = 0;
  const missing = [];

  const sections = {};

  for (const { path, value: enVal } of fromLeaves) {
    const toVal = toMap[path];
    const isPresent = (typeof toVal === 'string' && toVal.trim().length > 0) || (Array.isArray(toVal) && toVal.length > 0 && toVal.every(i => typeof i === 'string' && i.trim().length > 0));
    if (isPresent) present++;
    if (typeof toVal === 'string' && toVal !== enVal) different++;
    const top = path.split('.')[0];
    sections[top] = sections[top] || { total: 0, present: 0, different: 0 };
    sections[top].total++;
    if (isPresent) sections[top].present++;
    if (typeof toVal === 'string' && toVal !== enVal) sections[top].different++;
    if (!isPresent) missing.push({ path, en: enVal, to: toVal });
  }

  // Count 'extra' keys that are present in toMap but not in fromMap (only comparing leaf paths)
  const fromPaths = new Set(fromLeaves.map(x => x.path));
  const toLeaves = flattenMapToLeaves(toMap);
  const extra = toLeaves.filter(x => !fromPaths.has(x.path));

  return { total, present, different, missing, sections, extra };
}

function printReport(res, opts = {}) {
  const { total, present, different, missing, sections, extra } = res;
  console.log('i18n coverage report (src TOML)');
  console.log('--------------------------------');
  console.log(`Total strings in source: ${total}`);
  console.log(`Translated (target present): ${present} (${pct(present, total)})`);
  console.log(`Actually different from source: ${different} (${pct(different, total)})`);
  console.log('');
  console.log('By top-level section:');
  Object.keys(sections).sort().forEach(sec => {
    const d = sections[sec];
    console.log(` - ${sec}: ${d.present}/${d.total} (${pct(d.present, d.total)}) present; ${d.different} different`);
  });
  console.log('');
  if (missing.length > 0) {
    const maxShow = Number.isInteger(opts.maxMissing) ? opts.maxMissing : 50;
    console.log(`Missing translations (${missing.length}):`);
    missing.slice(0, maxShow).forEach(m => {
      console.log(` - ${m.path}  (source: "${m.en}")`);
    });
    if (missing.length > maxShow) console.log(` ...and ${missing.length - maxShow} more`);
  } else {
    console.log('No missing translations detected.');
  }
  if (extra.length > 0) {
    console.log('');
    console.log('Extra translations in target (not present in source):');
    extra.slice(0, 50).forEach(e => {
      console.log(` - ${e.path} (target: "${e.value}")`);
    });
    if (extra.length > 50) console.log(` ...and ${extra.length - 50} more`);
  }
}

function buildLeafMap(map) {
  // Convert keys like 'k[0]' back into map entries so analyze can find them easily.
  // We store leaves into a map keyed by leaf path (same format as flattenMapToLeaves uses).
  const out = {};
  Object.keys(map).forEach(k => {
    const v = map[k];
    if (typeof v === 'string') out[k] = v;
    else if (Array.isArray(v)) {
      v.forEach((el, idx) => {
        out[`${k}[${idx}]`] = el;
      });
    }
  });
  return out;
}

// Main
(function main() {
  const args = parseArgs(process.argv.slice(2));
  if (args.help || args.h) usageAndExit(0);

  const fromLang = args.from || 'en';
  const toLang = args.to || 'zh';
  const dir = args.dir ? path.resolve(args.dir) : path.join(__dirname, '..', 'src', 'i18n');
  const failUnder = args['fail-under'] ? Number(args['fail-under']) : null;
  const fmtJson = args.format === 'json' || args.json || args['format=json'];
  const maxMissing = args['max-missing'] ? Number(args['max-missing']) : 50;

  const fromPath = path.join(dir, `${fromLang}.toml`);
  const toPath = path.join(dir, `${toLang}.toml`);

  try {
    if (!fs.existsSync(fromPath)) {
      console.error(`Source file not found: ${fromPath}`);
      process.exit(1);
    }
    if (!fs.existsSync(toPath)) {
      console.error(`Target file not found: ${toPath}`);
      process.exit(1);
    }

    const fromContent = fs.readFileSync(fromPath, 'utf8');
    const toContent = fs.readFileSync(toPath, 'utf8');

    const fromMap = parseTomlStrings(fromContent);
    const toMapRaw = parseTomlStrings(toContent);

    // For ease of analysis, expand arrays into per-index leaf keys.
    const fromLeafMap = buildLeafMap(fromMap);
    const toLeafMap = buildLeafMap(toMapRaw);

    const result = analyze(fromLeafMap, toLeafMap);

    if (fmtJson) {
      const out = {
        total: result.total,
        present: result.present,
        different: result.different,
        percent_present: Number(((result.present / Math.max(result.total,1)) * 100).toFixed(1)),
        percent_different: Number(((result.different / Math.max(result.total,1)) * 100).toFixed(1)),
        missing_count: result.missing.length,
        extra_count: result.extra.length,
        sections: result.sections,
        missing: result.missing.slice(0, maxMissing),
      };
      console.log(JSON.stringify(out, null, 2));
    } else {
      printReport(result, { maxMissing });
    }

    if (failUnder !== null && !Number.isNaN(failUnder)) {
      const presentPct = (result.present / Math.max(result.total, 1)) * 100;
      if (presentPct < failUnder) {
        console.error(`Coverage ${presentPct.toFixed(1)}% is under threshold ${failUnder}% -> failing`);
        process.exit(2);
      }
    }

    process.exit(0);
  } catch (err) {
    console.error('Error:', err && err.message ? err.message : err);
    process.exit(1);
  }
})();
