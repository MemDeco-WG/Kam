#!/usr/bin/env node
/**
 * i18n coverage script
 *
 * Scans `Kam/KamWEBUI/src/main.ts` for a top-level `const messages = { ... }`
 * object and computes translation coverage from `en` -> `zh`.
 *
 * Usage:
 *   node Kam/scripts/i18n-coverage.js
 *
 * Outputs:
 *   - total number of translatable strings in `en`
 *   - how many are present in `zh` (non-empty)
 *   - how many are actually different from the English string (i.e. likely translated)
 *   - per-section and per-command breakdowns
 *   - list of missing keys (sample)
 *
 * Notes:
 *  - This uses a light-weight parser to extract the object literal; it
 *    intentionally ignores braces inside strings/comments.
 *  - The script treats any string leaf (including array string entries) as a
 *    translatable unit.
 */
const fs = require('fs');
const vm = require('vm');
const path = require('path');

const FILE = path.join(__dirname, '..', 'KamWEBUI', 'src', 'main.ts');

function extractMessagesSource(content) {
  const needle = 'const messages =';
  const idx = content.indexOf(needle);
  if (idx === -1) throw new Error('Could not find "const messages ="');
  const start = content.indexOf('{', idx);
  if (start === -1) throw new Error('Could not find opening "{" for messages');

  let i = start;
  let depth = 0;
  let inSingle = false;
  let inDouble = false;
  let inTemplate = false;
  let inLineComment = false;
  let inBlockComment = false;
  let prev = '';

  for (; i < content.length; i++) {
    const ch = content[i];
    const next = content[i + 1];

    if (inLineComment) {
      if (ch === '\n') inLineComment = false;
      prev = ch;
      continue;
    }

    if (inBlockComment) {
      if (ch === '*' && next === '/') {
        inBlockComment = false;
        i++; // skip '/'
        prev = '/';
        continue;
      }
      prev = ch;
      continue;
    }

    if (inSingle) {
      if (ch === "'" && prev !== '\\') inSingle = false;
      prev = ch;
      continue;
    }

    if (inDouble) {
      if (ch === '"' && prev !== '\\') inDouble = false;
      prev = ch;
      continue;
    }

    if (inTemplate) {
      if (ch === '`' && prev !== '\\') inTemplate = false;
      prev = ch;
      continue;
    }

    // not in any string/comment
    if (ch === '/' && next === '/') {
      inLineComment = true;
      i++;
      prev = '/';
      continue;
    }
    if (ch === '/' && next === '*') {
      inBlockComment = true;
      i++;
      prev = '/';
      continue;
    }
    if (ch === "'") {
      inSingle = true;
      prev = ch;
      continue;
    }
    if (ch === '"') {
      inDouble = true;
      prev = ch;
      continue;
    }
    if (ch === '`') {
      inTemplate = true;
      prev = ch;
      continue;
    }

    if (ch === '{') depth++;
    if (ch === '}') {
      depth--;
      if (depth === 0) {
        return content.slice(start, i + 1);
      }
    }

    prev = ch;
  }

  throw new Error('Unmatched "{" when parsing messages object');
}

function parseMessages(objLiteralStr) {
  // Wrap and evaluate in a safe vm context (no closures, only object literal)
  const code = '(function(){ return ' + objLiteralStr + ' })()';
  return vm.runInNewContext(code, {}, {timeout: 1000});
}

function collectStrings(obj, prefix = '') {
  const out = [];
  if (typeof obj === 'string') {
    out.push({path: prefix, value: obj});
  } else if (Array.isArray(obj)) {
    obj.forEach((el, idx) => {
      const p = prefix ? `${prefix}[${idx}]` : `[${idx}]`;
      if (typeof el === 'string') out.push({path: p, value: el});
      else out.push(...collectStrings(el, p));
    });
  } else if (obj && typeof obj === 'object') {
    Object.keys(obj).forEach(k => {
      const p = prefix ? `${prefix}.${k}` : k;
      out.push(...collectStrings(obj[k], p));
    });
  }
  return out;
}

function getValueAtPath(obj, pathStr) {
  const re = /([^[\].]+)|\[(\d+)\]/g;
  let m;
  let cur = obj;
  while ((m = re.exec(pathStr)) !== null) {
    const prop = m[1] !== undefined ? m[1] : Number(m[2]);
    if (cur == null) return undefined;
    cur = cur[prop];
  }
  return cur;
}

function pct(n, total) {
  if (total === 0) return '100%';
  return ((n / total) * 100).toFixed(1) + '%';
}

function analyze(messages, from = 'en', to = 'zh') {
  if (!messages[from]) throw new Error('No `' + from + '` locale found');
  const fromStrings = collectStrings(messages[from]);
  const toObj = messages[to] || {};

  const total = fromStrings.length;
  let present = 0;
  let different = 0;
  const missing = [];

  for (const {path, value: fv} of fromStrings) {
    const tv = getValueAtPath(toObj, path);
    const isPresent = typeof tv === 'string' && tv.trim().length > 0;
    if (isPresent) present++;
    if (isPresent && tv !== fv) different++;
    if (!isPresent) missing.push({path, en: fv, zh: tv});
  }

  const sections = {};
  for (const {path, value: fv} of fromStrings) {
    const top = path.split('.')[0];
    sections[top] = sections[top] || {total: 0, present: 0, different: 0};
    sections[top].total++;
    const tv = getValueAtPath(toObj, path);
    const isPresent = typeof tv === 'string' && tv.trim().length > 0;
    if (isPresent) sections[top].present++;
    if (isPresent && tv !== fv) sections[top].different++;
  }

  const perCommand = {};
  fromStrings.forEach(({path, value: fv}) => {
    if (path.startsWith('commands.')) {
      const rest = path.slice('commands.'.length);
      const cmd = rest.split('.')[0];
      perCommand[cmd] = perCommand[cmd] || {total: 0, present: 0, different: 0};
      perCommand[cmd].total++;
      const tv = getValueAtPath(toObj, path);
      const isPresent = typeof tv === 'string' && tv.trim().length > 0;
      if (isPresent) perCommand[cmd].present++;
      if (isPresent && tv !== fv) perCommand[cmd].different++;
    }
  });

  const toStrings = collectStrings(toObj);
  const fromPaths = new Set(fromStrings.map(x => x.path));
  const extra = toStrings.filter(x => !fromPaths.has(x.path));

  return {total, present, different, missing, sections, perCommand, extra};
}

function printReport(r) {
  console.log('i18n coverage report (en -> zh)');
  console.log('--------------------------------');
  console.log(`Total strings in en: ${r.total}`);
  console.log(`Translated (zh present): ${r.present} (${pct(r.present, r.total)})`);
  console.log(`Actually different from en: ${r.different} (${pct(r.different, r.total)})`);
  console.log('');
  console.log('By section:');
  Object.keys(r.sections).sort().forEach(sec => {
    const d = r.sections[sec];
    console.log(` - ${sec}: ${d.present}/${d.total} (${pct(d.present, d.total)}) present; ${d.different} different`);
  });
  console.log('');
  console.log('Commands breakdown:');
  Object.keys(r.perCommand).sort().forEach(cmd => {
    const d = r.perCommand[cmd];
    console.log(` - ${cmd}: ${d.present}/${d.total} (${pct(d.present, d.total)}) present; ${d.different} different`);
  });
  console.log('');
  if (r.missing.length > 0) {
    console.log(`Missing translations (${r.missing.length}):`);
    r.missing.slice(0, 200).forEach(m => {
      console.log(` - ${m.path}  (en: "${m.en}")`);
    });
    if (r.missing.length > 200) console.log(` ...and ${r.missing.length - 200} more`);
  } else {
    console.log('No missing translations detected.');
  }
  if (r.extra.length > 0) {
    console.log('');
    console.log('Extra translations present in zh that are not in en (may be stale):');
    r.extra.slice(0, 50).forEach(e => {
      console.log(` - ${e.path} (zh: "${e.value}")`);
    });
    if (r.extra.length > 50) console.log(` ...and ${r.extra.length - 50} more`);
  }
}

function main() {
  const content = fs.readFileSync(FILE, 'utf8');
  const objStr = extractMessagesSource(content);
  const messages = parseMessages(objStr);
  const r = analyze(messages, 'en', 'zh');
  printReport(r);
}

if (require.main === module) {
  try {
    main();
  } catch (err) {
    console.error('Error computing i18n coverage:', err && err.message ? err.message : err);
    process.exit(1);
  }
}
