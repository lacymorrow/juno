#!/usr/bin/env node
/**
 * Generate TypeScript constants from the Rust constants tree.
 *
 * `src-tauri/src/constants` is the single source of truth for every shared
 * name and default. This script reads it and writes
 * `src/lib/constants.generated.ts`, which the whole frontend imports.
 *
 * It holds itself to two rules, and they are separate rules:
 *
 *   1. A value that reaches the frontend is the value Rust holds. There is no
 *      alternative in the grammar below that can match a constant and come
 *      away with something that is not its value. The old parser had one: a
 *      bare `(\w+)` meant for `true` and `false` matched the first word of
 *      *any* expression, so `pub const X: &str = crate::a::b::C;` shipped the
 *      string 'crate'.
 *
 *   2. Nothing is skipped quietly. Every `pub const` and `pub static` under
 *      `src-tauri/src/constants` is parsed, and anything this file cannot turn
 *      into a value stops the build naming the file, the line, the constant
 *      and the reason. Silence was the real defect; a wrong value was only how
 *      it showed up. The old parser dropped, with no message, every negative
 *      number, every tuple, every multi-line array, every `[T; N]` array and
 *      every constant defined in terms of another one: 24 constants in all.
 *
 * Constants defined in terms of other constants are RESOLVED, not refused.
 * Refusing would have been simpler and no less honest, but authors were
 * already working around the old parser by hand-copying literals and pinning
 * them with Rust tests (see `defaults::PERMISSION_MODE` and
 * `defaults::DICTATION_INSERTION_MODE` in constants/settings.rs). Resolution
 * removes the reason for those copies. A reference that cannot be resolved to
 * exactly one constant inside this tree is an error, never a guess.
 *
 * Known limits, each of which is an error rather than a silent skip:
 *   - `pub use some::module::*;` inside a file whose constants are exported.
 *   - a constant whose `#[cfg]` variants are anything other than a single
 *     macOS / not-macOS pair.
 *   - a value form outside the grammar in `parseSum` (a function call such as
 *     `Duration::from_secs(5)`, a struct literal, a bitwise expression such as
 *     `COMMAND | SHIFT`, a reference out of this tree). The grammar covers
 *     every form the tree holds today; a new one is a deliberate decision, so
 *     inline a literal or teach `parsePrimary` the form.
 *
 * `#[cfg(test)]` modules and items are skipped on purpose: test fixtures are
 * not product values.
 *
 * Run with `bun run generate-constants`.
 */

import fs from 'fs';
import path from 'path';
import { fileURLToPath } from 'url';

const SCRIPT_DIR = path.dirname(fileURLToPath(import.meta.url));
const REPO_ROOT = path.resolve(SCRIPT_DIR, '..');
const RUST_CONSTANTS_DIR = path.join(REPO_ROOT, 'src-tauri/src/constants');
const TS_OUTPUT_FILE = path.join(REPO_ROOT, 'src/lib/constants.generated.ts');

/** The crate-relative module name of the constants directory itself. */
const ROOT_MODULE = 'constants';

/**
 * Thrown for anything the generator will not guess at. The message always
 * names the file, the line, the constant and the reason.
 */
export class ConstantsError extends Error {
    constructor(message) {
        super(message);
        this.name = 'ConstantsError';
    }
}

function fail(where, reason, hint) {
    const lines = [`${where}: ${reason}`];
    if (hint) lines.push(`  ${hint}`);
    throw new ConstantsError(lines.join('\n'));
}

// ---------------------------------------------------------------------------
// Lexer
// ---------------------------------------------------------------------------

const INT_TYPES = new Set([
    'u8', 'u16', 'u32', 'u64', 'u128', 'usize',
    'i8', 'i16', 'i32', 'i64', 'i128', 'isize',
]);
const FLOAT_TYPES = new Set(['f32', 'f64']);
const NUMERIC_SUFFIXES = new Set([...INT_TYPES, ...FLOAT_TYPES]);

const TWO_CHAR_PUNCT = new Set(['::', '->', '=>', '..', '==', '!=', '<=', '>=', '&&', '||']);

/**
 * Turn Rust source into tokens. Comments are dropped here, inside the one
 * piece of code that knows where a string ends, so a `//` in a URL or a `{` in
 * a doc comment can never be mistaken for code.
 */
function lex(source, file) {
    const tokens = [];
    const n = source.length;
    let i = 0;
    let line = 1;

    const push = (kind, start, extra) => {
        tokens.push({ kind, start, end: i, line, raw: source.slice(start, i), ...extra });
    };

    while (i < n) {
        const c = source[i];

        if (c === '\n') { line++; i++; continue; }
        if (c === ' ' || c === '\t' || c === '\r' || c === '\f') { i++; continue; }

        // Comments.
        if (c === '/' && source[i + 1] === '/') {
            while (i < n && source[i] !== '\n') i++;
            continue;
        }
        if (c === '/' && source[i + 1] === '*') {
            let depth = 1;
            i += 2;
            while (i < n && depth > 0) {
                if (source[i] === '/' && source[i + 1] === '*') { depth += 1; i += 2; }
                else if (source[i] === '*' && source[i + 1] === '/') { depth -= 1; i += 2; }
                else { if (source[i] === '\n') line++; i++; }
            }
            if (depth > 0) fail(`${file}:${line}`, 'unterminated block comment');
            continue;
        }

        // Raw strings: r"...", r#"..."#, br#"..."#.
        const raw = /^(b?r)(#*)"/.exec(source.slice(i, i + 24));
        if (raw) {
            const hashes = raw[2];
            const bodyStart = i + raw[0].length;
            const terminator = `"${hashes}`;
            const close = source.indexOf(terminator, bodyStart);
            if (close === -1) fail(`${file}:${line}`, 'unterminated raw string');
            const value = source.slice(bodyStart, close);
            const start = i;
            i = close + terminator.length;
            line += countNewlines(value);
            push('string', start, { value });
            continue;
        }

        // Strings.
        if (c === '"' || (c === 'b' && source[i + 1] === '"')) {
            const start = i;
            i += c === '"' ? 1 : 2;
            let value = '';
            while (i < n && source[i] !== '"') {
                if (source[i] === '\\') {
                    const [decoded, next] = decodeEscape(source, i, file, line);
                    value += decoded;
                    i = next;
                } else {
                    if (source[i] === '\n') line++;
                    value += source[i];
                    i++;
                }
            }
            if (i >= n) fail(`${file}:${line}`, 'unterminated string literal');
            i++; // closing quote
            push('string', start, { value });
            continue;
        }

        // Character literals and lifetimes.
        if (c === "'") {
            const charMatch = /^'(\\.[^']*|[^'\\])'/.exec(source.slice(i, i + 16));
            const start = i;
            if (charMatch) {
                i += charMatch[0].length;
                push('char', start);
            } else {
                const lifetime = /^'[A-Za-z_]\w*/.exec(source.slice(i, i + 64));
                if (!lifetime) fail(`${file}:${line}`, "stray ' in source");
                i += lifetime[0].length;
                push('lifetime', start);
            }
            continue;
        }

        // Numbers.
        if (c >= '0' && c <= '9') {
            const match = /^(?:0x[0-9a-fA-F_]+|0b[01_]+|0o[0-7_]+|[0-9][0-9_]*(?:\.[0-9][0-9_]*)?(?:[eE][+-]?[0-9]+)?)(?:[A-Za-z][A-Za-z0-9]*)?/
                .exec(source.slice(i));
            const start = i;
            i += match[0].length;
            push('number', start);
            continue;
        }

        // Identifiers and keywords.
        if (/[A-Za-z_]/.test(c)) {
            const match = /^[A-Za-z_]\w*/.exec(source.slice(i));
            const start = i;
            i += match[0].length;
            push('ident', start, { value: match[0] });
            continue;
        }

        // Punctuation.
        const two = source.slice(i, i + 2);
        const start = i;
        if (TWO_CHAR_PUNCT.has(two)) i += 2;
        else i += 1;
        push('punct', start, { value: source.slice(start, i) });
    }

    return tokens;
}

function countNewlines(text) {
    let count = 0;
    for (const ch of text) if (ch === '\n') count++;
    return count;
}

function decodeEscape(source, at, file, line) {
    const c = source[at + 1];
    switch (c) {
        case 'n': return ['\n', at + 2];
        case 'r': return ['\r', at + 2];
        case 't': return ['\t', at + 2];
        case '0': return ['\0', at + 2];
        case '\\': return ['\\', at + 2];
        case '"': return ['"', at + 2];
        case "'": return ["'", at + 2];
        case '\n': {
            // A trailing backslash joins lines and eats leading whitespace.
            let j = at + 2;
            while (j < source.length && /\s/.test(source[j])) j++;
            return ['', j];
        }
        case 'x': {
            const hex = source.slice(at + 2, at + 4);
            if (!/^[0-9a-fA-F]{2}$/.test(hex)) fail(`${file}:${line}`, `bad \\x escape '\\x${hex}'`);
            return [String.fromCharCode(parseInt(hex, 16)), at + 4];
        }
        case 'u': {
            const match = /^\\u\{([0-9a-fA-F_]{1,6})\}/.exec(source.slice(at));
            if (!match) fail(`${file}:${line}`, 'bad \\u escape');
            return [String.fromCodePoint(parseInt(match[1].replace(/_/g, ''), 16)), at + match[0].length];
        }
        default:
            return fail(`${file}:${line}`, `unknown string escape '\\${c}'`);
    }
}

/**
 * Pair every bracket with its partner up front. An unbalanced bracket is an
 * error here rather than a module body that quietly ends in the wrong place,
 * which is how the old brace counter behaved when a string held a lone brace.
 */
function matchBrackets(tokens, file) {
    const open = { '{': '}', '[': ']', '(': ')' };
    const close = new Set(['}', ']', ')']);
    const pairs = new Map();
    const stack = [];
    tokens.forEach((token, index) => {
        if (token.kind !== 'punct') return;
        if (open[token.value]) {
            stack.push(index);
            return;
        }
        if (close.has(token.value)) {
            const start = stack.pop();
            if (start === undefined) fail(`${file}:${token.line}`, `unmatched '${token.value}'`);
            const expected = open[tokens[start].value];
            if (expected !== token.value) {
                fail(`${file}:${token.line}`, `'${tokens[start].value}' on line ${tokens[start].line} closed by '${token.value}'`);
            }
            pairs.set(start, index);
            pairs.set(index, start);
        }
    });
    if (stack.length) {
        const token = tokens[stack[stack.length - 1]];
        fail(`${file}:${token.line}`, `'${token.value}' is never closed`);
    }
    return pairs;
}

// ---------------------------------------------------------------------------
// Item parsing
// ---------------------------------------------------------------------------

const GROUP_OPENERS = new Set(['{', '[', '(']);

function isPunct(token, value) {
    return token && token.kind === 'punct' && token.value === value;
}

function isIdent(token, value) {
    return token && token.kind === 'ident' && token.value === value;
}

/**
 * Walk the items of one module body, collecting constants, `use` statements
 * and nested modules. Anything that is not one of those (functions, structs,
 * impls, macros) is stepped over as a whole item.
 */
function parseItems(ctx, from, to, modulePath) {
    const { tokens, pairs, file } = ctx;
    const module = ctx.moduleFor(modulePath);
    let i = from;

    while (i < to) {
        const attrs = [];

        // Attributes, inner (`#![...]`) and outer (`#[...]`).
        while (i < to && isPunct(tokens[i], '#')) {
            let bracket = i + 1;
            if (isPunct(tokens[bracket], '!')) bracket += 1;
            if (!isPunct(tokens[bracket], '[')) {
                fail(`${file}:${tokens[i].line}`, "'#' is not followed by an attribute");
            }
            const end = pairs.get(bracket);
            attrs.push(ctx.source.slice(tokens[bracket + 1].start, tokens[end - 1].end));
            i = end + 1;
        }

        if (i >= to) break;

        let isPub = false;
        if (isIdent(tokens[i], 'pub')) {
            isPub = true;
            i += 1;
            if (isPunct(tokens[i], '(')) i = pairs.get(i) + 1;
        }

        const token = tokens[i];
        if (!token) break;

        const cfg = classifyCfg(attrs);
        const skipForTest = cfg === 'test';

        if (isIdent(token, 'use')) {
            const end = findAtDepth(ctx, i, to, ';');
            if (!skipForTest && end > i + 1) {
                const text = ctx.source.slice(tokens[i + 1].start, tokens[end - 1].end);
                recordUse(ctx, module, text, isPub, token.line);
            }
            i = end + 1;
            continue;
        }

        if (isIdent(token, 'mod')) {
            const nameToken = tokens[i + 1];
            if (!nameToken || nameToken.kind !== 'ident') {
                fail(`${file}:${token.line}`, 'module has no name');
            }
            const body = tokens[i + 2];
            if (isPunct(body, ';')) { i += 3; continue; } // `mod foo;` file module
            if (!isPunct(body, '{')) fail(`${file}:${token.line}`, `module ${nameToken.value} has no body`);
            const end = pairs.get(i + 2);
            if (!skipForTest) {
                const childPath = [...modulePath, nameToken.value];
                ctx.moduleFor(childPath).isPub = isPub;
                parseItems(ctx, i + 3, end, childPath);
            }
            i = end + 1;
            continue;
        }

        if (isIdent(token, 'const') || isIdent(token, 'static')) {
            const record = parseConst(ctx, i, to, modulePath, { isPub, cfg, attrs });
            if (!skipForTest) module.declare(record);
            i = record.afterIndex;
            continue;
        }

        i = skipItem(ctx, i, to);
    }
}

function parseConst(ctx, at, to, modulePath, { isPub, cfg, attrs }) {
    const { tokens, file } = ctx;
    let i = at + 1;
    if (isIdent(tokens[i], 'mut')) i += 1;
    const nameToken = tokens[i];
    if (!nameToken || nameToken.kind !== 'ident') {
        fail(`${file}:${tokens[at].line}`, 'constant has no name');
    }
    i += 1;
    if (!isPunct(tokens[i], ':')) {
        fail(`${file}:${nameToken.line}`, `constant ${nameToken.value} has no type annotation`);
    }
    i += 1;

    const typeStart = i;
    i = findAtDepth(ctx, i, to, '=');
    const typeText = ctx.source.slice(tokens[typeStart].start, tokens[i - 1].end).replace(/\s+/g, '');
    i += 1;

    const valueStart = i;
    const valueEnd = findAtDepth(ctx, i, to, ';');

    return {
        file,
        line: nameToken.line,
        name: nameToken.value,
        modulePath,
        typeText,
        valueStart,
        valueEnd,
        valueText: ctx.source.slice(tokens[valueStart].start, tokens[valueEnd - 1].end).replace(/\s+/g, ' '),
        isPub,
        cfg,
        attrs,
        afterIndex: valueEnd + 1,
    };
}

/** Index of the first token with `value` that is not inside a bracket group. */
function findAtDepth(ctx, from, to, value) {
    const { tokens, pairs, file } = ctx;
    let i = from;
    while (i < to) {
        const token = tokens[i];
        if (token.kind === 'punct') {
            if (GROUP_OPENERS.has(token.value)) { i = pairs.get(i) + 1; continue; }
            if (token.value === value) return i;
        }
        i += 1;
    }
    fail(`${file}:${tokens[from] ? tokens[from].line : '?'}`, `expected '${value}' and did not find one`);
    return to;
}

/** Step over an item this generator does not care about. */
function skipItem(ctx, from, to) {
    const { tokens, pairs } = ctx;
    let i = from;
    while (i < to) {
        const token = tokens[i];
        if (token.kind === 'punct') {
            if (token.value === '{') {
                let after = pairs.get(i) + 1;
                if (isPunct(tokens[after], ';')) after += 1;
                return after;
            }
            if (token.value === '[' || token.value === '(') { i = pairs.get(i) + 1; continue; }
            if (token.value === ';') return i + 1;
        }
        i += 1;
    }
    return to;
}

/**
 * Which platform an item belongs to. Only the macOS / not-macOS pair the
 * settings defaults use is understood; any other `#[cfg]` keeps its own text
 * so the export step can refuse it by name instead of picking one arm.
 */
function classifyCfg(attrs) {
    for (const attr of attrs) {
        const normalized = attr.replace(/\s+/g, '');
        if (!normalized.startsWith('cfg(')) continue;
        if (normalized === 'cfg(test)') return 'test';
        if (normalized === 'cfg(target_os="macos")') return 'macos';
        if (normalized === 'cfg(not(target_os="macos"))') return 'other';
        return `cfg:${normalized}`;
    }
    return null;
}

function recordUse(ctx, module, rawText, isPub, line) {
    // `use a::b::{C, D};`, `use a::b::*;`, `use a::b::C;`, `use a::b as c;`
    const text = rawText.replace(/\s+/g, ' ').trim();
    const braceAt = text.indexOf('{');
    if (braceAt !== -1) {
        const inner = text.slice(braceAt + 1, text.lastIndexOf('}'));
        if (inner.includes('{')) {
            fail(
                `${ctx.file}:${line}`,
                `'use ${text}' nests braces, which this generator does not take apart`,
                'write the imports out one per line.',
            );
        }
        const prefix = text.slice(0, braceAt).replace(/\s*::\s*$/, '');
        for (const name of inner.split(',').map((s) => s.trim()).filter(Boolean)) {
            addUse(module, prefix, name, isPub, line, ctx);
        }
        return;
    }
    const lastSep = text.lastIndexOf('::');
    if (lastSep === -1) return; // `use foo;` - nothing we can resolve or need
    addUse(module, text.slice(0, lastSep), text.slice(lastSep + 2), isPub, line, ctx);
}

function addUse(module, prefix, name, isPub, line, ctx) {
    const segments = prefix.split('::').map((s) => s.trim()).filter(Boolean);
    if (name.trim() === '*') {
        module.uses.push({ kind: 'glob', segments, isPub, line, file: ctx.file });
        return;
    }
    const [imported, alias] = name.split(/\s+as\s+/);
    module.uses.push({
        kind: 'name',
        segments,
        name: imported.trim(),
        alias: (alias || imported).trim(),
        isPub,
        line,
        file: ctx.file,
    });
}

// ---------------------------------------------------------------------------
// The tree
// ---------------------------------------------------------------------------

function listRustFiles(dir, base = '') {
    const entries = fs.readdirSync(dir, { withFileTypes: true });
    const files = [];
    for (const entry of entries.sort((a, b) => a.name.localeCompare(b.name))) {
        const rel = base ? `${base}/${entry.name}` : entry.name;
        if (entry.isDirectory()) files.push(...listRustFiles(path.join(dir, entry.name), rel));
        else if (entry.name.endsWith('.rs')) files.push(rel);
    }
    return files;
}

/** `settings.rs` -> constants::settings, `mod.rs` -> constants, `platform/macos.rs` -> constants::platform::macos */
function modulePathForFile(relPath) {
    const parts = relPath.replace(/\.rs$/, '').split('/');
    if (parts[parts.length - 1] === 'mod') parts.pop();
    return [ROOT_MODULE, ...parts];
}

const key = (modulePath) => modulePath.join('::');

class Module {
    constructor(modulePath) {
        this.path = modulePath;
        this.consts = new Map(); // name -> [record]
        this.uses = [];
        this.reexports = [];
        this.isPub = true;
    }

    declare(record) {
        const existing = this.consts.get(record.name) || [];
        for (const other of existing) {
            if (other.cfg === record.cfg) {
                fail(
                    `${record.file}:${record.line}`,
                    `${key(record.modulePath)}::${record.name} is declared twice with the same #[cfg]`,
                    'two declarations of one name mean one of them is dead; remove it.',
                );
            }
        }
        existing.push(record);
        this.consts.set(record.name, existing);
    }
}

/**
 * Read every .rs file under the constants directory and build the module tree.
 */
function buildTree() {
    const files = listRustFiles(RUST_CONSTANTS_DIR);
    if (files.length === 0) fail(RUST_CONSTANTS_DIR, 'no Rust files found; the constants tree moved or is unreadable');
    return buildTreeFrom(files.map((relPath) => ({
        relPath,
        source: fs.readFileSync(path.join(RUST_CONSTANTS_DIR, relPath), 'utf8'),
    })));
}

function buildTreeFrom(entries) {
    const modules = new Map();
    const sources = new Map();
    const moduleFor = (modulePath) => {
        const id = key(modulePath);
        if (!modules.has(id)) modules.set(id, new Module(modulePath));
        return modules.get(id);
    };

    const files = entries.map((entry) => entry.relPath);

    for (const { relPath, source } of entries) {
        const tokens = lex(source, relPath);
        const pairs = matchBrackets(tokens, relPath);
        const modulePath = modulePathForFile(relPath);
        moduleFor(modulePath);
        const ctx = { tokens, pairs, source, file: relPath, moduleFor };
        sources.set(relPath, ctx);
        parseItems(ctx, 0, tokens.length, modulePath);
    }

    return { modules, moduleFor, files, sources };
}

// ---------------------------------------------------------------------------
// Reference resolution
// ---------------------------------------------------------------------------

function resolveModulePath(tree, segments, fromModulePath) {
    if (segments.length === 0) return fromModulePath;

    const [head, ...rest] = segments;

    if (head === 'crate') {
        if (rest[0] !== ROOT_MODULE) return null; // outside the constants tree
        return [ROOT_MODULE, ...rest.slice(1)];
    }
    if (head === 'self') return resolveModulePath(tree, rest, fromModulePath);
    if (head === 'super') {
        if (fromModulePath.length <= 1) return null;
        return resolveModulePath(tree, rest, fromModulePath.slice(0, -1));
    }

    const candidates = [
        [...fromModulePath, ...segments],
        [ROOT_MODULE, ...segments],
    ];
    for (let depth = fromModulePath.length - 1; depth >= 1; depth -= 1) {
        candidates.push([...fromModulePath.slice(0, depth), ...segments]);
    }
    for (const candidate of candidates) {
        if (tree.modules.has(key(candidate))) return candidate;
    }
    return null;
}

/**
 * Find the constant a reference names. Returns its records (one, or a
 * platform pair). Anything other than exactly one answer is an error.
 */
function lookupConst(tree, segments, fromModulePath, loc, forConst) {
    const name = segments[segments.length - 1];
    const prefix = segments.slice(0, -1);

    if (prefix.length > 0) {
        const modulePath = resolveModulePath(tree, prefix, fromModulePath);
        if (!modulePath) {
            fail(
                loc,
                `${forConst} is defined as '${segments.join('::')}', which the generator cannot resolve`,
                'it names something outside src-tauri/src/constants; inline the literal instead.',
            );
        }
        const module = tree.modules.get(key(modulePath));
        const found = module && module.consts.get(name);
        if (!found) {
            fail(loc, `${forConst} is defined as '${segments.join('::')}', and ${key(modulePath)} has no constant '${name}'`);
        }
        return found;
    }

    const module = tree.modules.get(key(fromModulePath));

    const own = module && module.consts.get(name);
    if (own) return own;

    const explicit = (module ? module.uses : []).filter((u) => u.kind === 'name' && u.alias === name);
    for (const use of explicit) {
        const modulePath = resolveModulePath(tree, use.segments, fromModulePath);
        const target = modulePath && tree.modules.get(key(modulePath));
        const found = target && target.consts.get(use.name);
        if (found) return found;
    }

    const globHits = [];
    for (const use of (module ? module.uses : []).filter((u) => u.kind === 'glob')) {
        const modulePath = resolveModulePath(tree, use.segments, fromModulePath);
        const target = modulePath && tree.modules.get(key(modulePath));
        const found = target && target.consts.get(name);
        if (found) globHits.push({ use, found });
    }
    if (globHits.length === 1) return globHits[0].found;
    if (globHits.length > 1) {
        const where = globHits.map((hit) => hit.use.segments.join('::')).join(', ');
        fail(loc, `${forConst} refers to '${name}', which more than one glob import brings in (${where}); the generator will not pick one`);
    }

    fail(
        loc,
        `${forConst} refers to '${name}', and the generator cannot resolve it from ${key(fromModulePath)}`,
        'name the constant by its module path, or inline the literal.',
    );
    return null;
}

// ---------------------------------------------------------------------------
// Value evaluation
// ---------------------------------------------------------------------------

/**
 * The whole value grammar:
 *
 *   value   := sum
 *   sum     := product (('+' | '-') product)*
 *   product := unary (('*' | '/' | '%') unary)*
 *   unary   := ('-' | '&')* primary
 *   primary := string | number | 'true' | 'false'
 *            | '[' value (',' value)* ','? ']'          array or slice
 *            | '(' value (',' value)* ','? ')'          tuple, or grouping
 *            | path                                      another constant
 *
 * Arithmetic is folded only over numbers, and only with the declared type's
 * arithmetic: `u64 = 1000 / 60` is 16, not 16.666.
 */
function evaluateRecord(tree, record, stack = []) {
    if (record.cached !== undefined) return record.cached;

    const id = `${record.file}:${key(record.modulePath)}::${record.name}`;
    if (stack.includes(id)) {
        fail(`${record.file}:${record.line}`, `${record.name} is defined in terms of itself (${stack.join(' -> ')} -> ${id})`);
    }

    const ctx = tree.sources.get(record.file);
    const value = parseSum(tree, ctx, record, record.valueStart, record.valueEnd, [...stack, id]);
    record.cached = value;
    return value;
}

function loc(record, token) {
    return `${record.file}:${token ? token.line : record.line}`;
}

function parseSum(tree, ctx, record, from, to, stack) {
    let { value, next } = parseProduct(tree, ctx, record, from, to, stack);
    while (next < to) {
        const op = ctx.tokens[next];
        if (!isPunct(op, '+') && !isPunct(op, '-')) break;
        const right = parseProduct(tree, ctx, record, next + 1, to, stack);
        value = arithmetic(record, op.value, value, right.value, loc(record, op));
        next = right.next;
    }
    if (next < to) {
        fail(
            loc(record, ctx.tokens[next]),
            `cannot read the value of ${record.name}: unexpected '${ctx.tokens[next].raw}' in '${record.valueText}'`,
            'the generator reads literals, arrays, tuples, other constants and arithmetic over numbers. Inline a literal, or teach scripts/generate-ts-constants.js this form.',
        );
    }
    return value;
}

function parseProduct(tree, ctx, record, from, to, stack) {
    let { value, next } = parseUnary(tree, ctx, record, from, to, stack);
    while (next < to) {
        const op = ctx.tokens[next];
        if (!isPunct(op, '*') && !isPunct(op, '/') && !isPunct(op, '%')) break;
        const right = parseUnary(tree, ctx, record, next + 1, to, stack);
        value = arithmetic(record, op.value, value, right.value, loc(record, op));
        next = right.next;
    }
    return { value, next };
}

function parseUnary(tree, ctx, record, from, to, stack) {
    const token = ctx.tokens[from];
    if (isPunct(token, '&')) return parseUnary(tree, ctx, record, from + 1, to, stack);
    if (isPunct(token, '-')) {
        const inner = parseUnary(tree, ctx, record, from + 1, to, stack);
        if (typeof inner.value !== 'number') {
            fail(loc(record, token), `cannot negate a ${describe(inner.value)} in ${record.name}`);
        }
        return { value: -inner.value, next: inner.next };
    }
    return parsePrimary(tree, ctx, record, from, to, stack);
}

function parsePrimary(tree, ctx, record, from, to, stack) {
    const { tokens, pairs } = ctx;
    const token = tokens[from];
    if (!token || from >= to) {
        fail(`${record.file}:${record.line}`, `${record.name} has an empty value`);
    }

    if (token.kind === 'string') return { value: token.value, next: from + 1 };

    if (token.kind === 'number') return { value: parseNumber(record, token), next: from + 1 };

    if (token.kind === 'ident' && (token.value === 'true' || token.value === 'false')) {
        return { value: token.value === 'true', next: from + 1 };
    }

    if (isPunct(token, '[') || isPunct(token, '(')) {
        const close = pairs.get(from);
        const { items, trailingComma } = splitCommaList(ctx, from + 1, close);
        const values = items.map(([start, end]) => parseSum(tree, ctx, record, start, end, stack));
        if (isPunct(token, '(') && values.length === 1 && !trailingComma) {
            return { value: values[0], next: close + 1 }; // grouping, not a tuple
        }
        return { value: values, next: close + 1 };
    }

    if (token.kind === 'ident') {
        // A path: `NAME`, `module::NAME`, `crate::constants::module::NAME`.
        const segments = [token.value];
        let i = from + 1;
        while (isPunct(tokens[i], '::') && tokens[i + 1] && tokens[i + 1].kind === 'ident') {
            segments.push(tokens[i + 1].value);
            i += 2;
        }
        if (isPunct(tokens[i], '(') || isPunct(tokens[i], '{') || isPunct(tokens[i], '!')) {
            fail(
                loc(record, token),
                `cannot read the value of ${record.name}: '${record.valueText}' calls or constructs something`,
                'the generator reads values, it does not run Rust. Inline a literal.',
            );
        }
        const found = lookupConst(tree, segments, record.modulePath, loc(record, token), record.name);
        const live = found.filter((r) => r.cfg !== 'test');
        if (live.length !== 1) {
            fail(
                loc(record, token),
                `'${segments.join('::')}' has ${live.length} platform variants, so ${record.name} has no single value`,
                'inline the literal, or give this constant its own #[cfg] arms.',
            );
        }
        return { value: evaluateRecord(tree, live[0], stack), next: i };
    }

    return fail(
        loc(record, token),
        `cannot read the value of ${record.name}: unexpected '${token.raw}' in '${record.valueText}'`,
        'the generator reads literals, arrays, tuples, other constants and arithmetic over numbers.',
    );
}

function splitCommaList(ctx, from, to) {
    const { tokens, pairs } = ctx;
    const items = [];
    let start = from;
    let i = from;
    while (i < to) {
        const token = tokens[i];
        if (token.kind === 'punct' && GROUP_OPENERS.has(token.value)) { i = pairs.get(i) + 1; continue; }
        if (isPunct(token, ',')) {
            if (i > start) items.push([start, i]);
            start = i + 1;
        }
        i += 1;
    }
    if (start < to) items.push([start, to]);
    const trailingComma = to > from && isPunct(tokens[to - 1], ',');
    return { items, trailingComma };
}

const NUMBER_LITERAL = new RegExp(
    '^(0x[0-9a-fA-F_]+|0b[01_]+|0o[0-7_]+|[0-9][0-9_]*(?:\\.[0-9][0-9_]*)?(?:[eE][+-]?[0-9]+)?)' +
    `(${[...NUMERIC_SUFFIXES].join('|')})?$`,
);

function parseNumber(record, token) {
    const text = token.raw;
    const match = NUMBER_LITERAL.exec(text);
    if (!match) {
        fail(
            `${record.file}:${token.line}`,
            `'${text}' in ${record.name} is not a number literal this generator reads`,
            'decimal, hex, binary, octal and float literals with an optional primitive suffix are understood.',
        );
    }

    const clean = match[1].replace(/_/g, '');
    let value;
    if (/^0x/i.test(clean)) value = parseInt(clean.slice(2), 16);
    else if (/^0b/i.test(clean)) value = parseInt(clean.slice(2), 2);
    else if (/^0o/i.test(clean)) value = parseInt(clean.slice(2), 8);
    else value = Number(clean);

    if (!Number.isFinite(value)) {
        fail(`${record.file}:${token.line}`, `'${text}' in ${record.name} is not a finite number`);
    }
    if (Number.isInteger(value) && !Number.isSafeInteger(value)) {
        fail(
            `${record.file}:${token.line}`,
            `'${text}' in ${record.name} does not survive JavaScript's number type`,
            'a value this large cannot be a TypeScript number; keep it out of the generated constants.',
        );
    }
    return value;
}

function describe(value) {
    if (Array.isArray(value)) return 'list';
    return typeof value;
}

function arithmetic(record, op, left, right, where) {
    if (typeof left !== 'number' || typeof right !== 'number') {
        fail(where, `${record.name} applies '${op}' to a ${describe(left)} and a ${describe(right)}`);
    }
    const isInt = INT_TYPES.has(record.typeText);
    const isFloat = FLOAT_TYPES.has(record.typeText);
    if (!isInt && !isFloat && (op === '/' || op === '%')) {
        fail(
            where,
            `${record.name} divides, but its type '${record.typeText}' does not say whether that is integer or floating-point division`,
            'annotate it with a primitive numeric type, or inline the result.',
        );
    }
    let value;
    switch (op) {
        case '+': value = left + right; break;
        case '-': value = left - right; break;
        case '*': value = left * right; break;
        case '/':
            if (right === 0) fail(where, `${record.name} divides by zero`);
            value = isInt ? Math.trunc(left / right) : left / right;
            break;
        case '%':
            if (right === 0) fail(where, `${record.name} takes a remainder modulo zero`);
            value = left % right;
            break;
        default:
            return fail(where, `${record.name} uses the unsupported operator '${op}'`);
    }
    if (isInt && !Number.isSafeInteger(value)) {
        fail(where, `${record.name} works out to ${value}, which JavaScript cannot hold exactly`);
    }
    return value;
}

// ---------------------------------------------------------------------------
// Flattening a file into the keys the frontend sees
// ---------------------------------------------------------------------------

/**
 * One file's constants, keyed the way the generated file keys them: a
 * top-level constant by its own name, a constant inside `pub mod foo` as
 * `FOO_NAME`.
 */
function flattenFile(tree, relPath) {
    const filePath = modulePathForFile(relPath);
    const flat = {};
    const owners = new Map();

    const add = (prefix, module) => {
        for (const [name, records] of module.consts) {
            const live = records.filter((record) => record.cfg !== 'test' && record.isPub);
            if (live.length === 0) continue;
            const flatKey = prefix ? `${prefix}_${name}` : name;
            const previous = owners.get(flatKey);
            if (previous) {
                fail(
                    `${relPath}:${live[0].line}`,
                    `${key(module.path)}::${name} and ${previous} both become '${flatKey}' in the generated file`,
                    'rename one of them; otherwise one value silently replaces the other.',
                );
            }
            owners.set(flatKey, `${key(module.path)}::${name}`);
            flat[flatKey] = platformAwareValue(tree, relPath, flatKey, live);
        }
        for (const use of module.uses) {
            if (!use.isPub) continue;
            if (use.kind === 'glob') {
                fail(
                    `${relPath}:${use.line}`,
                    `'pub use ${use.segments.join('::')}::*' re-exports names this generator cannot enumerate`,
                    'name the constants you mean, so the generated file cannot quietly miss one.',
                );
            }
            const targetPath = resolveModulePath(tree, use.segments, module.path);
            const target = targetPath && tree.modules.get(key(targetPath));
            const records = target && target.consts.get(use.name);
            if (!records) continue; // a re-exported module, not a constant
            const flatKey = prefix ? `${prefix}_${use.alias}` : use.alias;
            if (owners.has(flatKey)) continue;
            owners.set(flatKey, `${use.segments.join('::')}::${use.name}`);
            flat[flatKey] = platformAwareValue(tree, relPath, flatKey, records.filter((r) => r.cfg !== 'test'));
        }
    };

    const fileModule = tree.modules.get(key(filePath));
    if (!fileModule) fail(relPath, 'file produced no module; the generator could not read it');

    // Nested modules first, then the file's own top-level constants. That is
    // the order the generated file has always had; keeping it means a diff of
    // the output shows only the constants that actually changed.
    for (const module of tree.modules.values()) {
        if (key(module.path.slice(0, filePath.length)) !== key(filePath)) continue;
        const depth = module.path.length - filePath.length;
        if (depth === 0) continue;
        if (depth > 1) {
            // A module inside a module would need a two-part prefix, and the
            // frontend's flat keys have no room for one. Loud rather than lost.
            fail(
                relPath,
                `module ${key(module.path)} is nested more than one level deep, and the generated file has no key shape for it`,
                'flatten the module, or extend the generator to name nested modules.',
            );
        }
        if (!module.isPub) continue;
        add(module.path[module.path.length - 1].toUpperCase(), module);
    }

    add('', fileModule);

    return flat;
}

/**
 * A constant with a macOS arm and a not-macOS arm becomes `{ macos, other }`,
 * which the emitter renders as a platform check. Any other set of `#[cfg]`
 * arms is an error rather than a coin flip.
 */
function platformAwareValue(tree, relPath, flatKey, records) {
    if (records.length === 1 && records[0].cfg === null) {
        return evaluateRecord(tree, records[0]);
    }
    const byCfg = new Map(records.map((record) => [record.cfg, record]));
    if (records.length === 2 && byCfg.has('macos') && byCfg.has('other')) {
        return {
            macos: evaluateRecord(tree, byCfg.get('macos')),
            other: evaluateRecord(tree, byCfg.get('other')),
        };
    }
    const arms = records.map((record) => record.cfg || 'no #[cfg]').join(', ');
    return fail(
        `${relPath}:${records[0].line}`,
        `'${flatKey}' is declared ${records.length} times (${arms}) and the generator only understands a macOS / not-macOS pair`,
        'give the frontend one value, or extend the generator to carry this platform split.',
    );
}

const isPlatformValue = (value) =>
    value !== null && typeof value === 'object' && !Array.isArray(value) && 'macos' in value && 'other' in value;

// ---------------------------------------------------------------------------
// Public parse entry point
// ---------------------------------------------------------------------------

/**
 * Files whose constants the frontend reads, and the name each one lands under.
 * A file in the constants directory that is not listed here is an error: a new
 * constants file must be a decision, not an omission.
 */
export const EXPORTED_FILES = {
    agent: 'agent.rs',
    api: 'api.rs',
    app: 'app.rs',
    audio: 'audio.rs',
    commands: 'commands.rs',
    events: 'events.rs',
    files: 'files.rs',
    memory: 'memory.rs',
    permissions: 'permissions.rs',
    ports: 'ports.rs',
    settings: 'settings.rs',
    timeouts: 'timeouts.rs',
    ui: 'ui.rs',
};

/**
 * Files that stay in Rust. Every one is still parsed, so a constant in here
 * that the generator cannot read still stops the build; it just does not reach
 * the frontend.
 */
export const BACKEND_ONLY_FILES = {
    'browser.rs': 'Chrome/CDP flags and URLs; the browser is driven from Rust.',
    'cli.rs': 'juno-cli argument and exit-code names; no frontend reads them.',
    'error_messages.rs': 'Rust-side format strings; the frontend writes its own copy.',
    'errors.rs': 'error codes and message templates raised in Rust. It was parsed before this change and the result thrown away: nothing emitted it.',
    'menus.rs': 'native menu item ids, used by the Tauri menu builder.',
    'mod.rs': 'module wiring and a few backend-only sizes.',
    'mouse.rs': 'mouse timing used by the Rust input layer.',
    'performance.rs': 'benchmark thresholds for Rust tests and telemetry.',
    'platform.rs': 'platform module wiring.',
    'platform/macos.rs': 'CoreGraphics key codes and event flags.',
    'text.rs': 'Rust-side string helpers.',
};

/**
 * Parse Rust source held in memory and return the flat constant map for the
 * first file, the way the frontend would see it. This is how the tests pin the
 * value grammar, including every form that has to raise.
 *
 * @param {string|Array<{relPath: string, source: string}>} input
 */
export function parseSources(input) {
    const entries = typeof input === 'string' ? [{ relPath: 'probe.rs', source: input }] : input;
    const tree = buildTreeFrom(entries);
    for (const module of tree.modules.values()) {
        for (const records of module.consts.values()) {
            for (const record of records) {
                if (record.cfg === 'test') continue;
                evaluateRecord(tree, record);
            }
        }
    }
    return flattenFile(tree, entries[0].relPath);
}

/**
 * Parse the Rust constants tree. Throws `ConstantsError` for anything it
 * cannot read; it never returns a partial answer.
 */
export function parseRustConstants() {
    const tree = buildTree();

    const known = new Set([...Object.values(EXPORTED_FILES), ...Object.keys(BACKEND_ONLY_FILES)]);
    for (const relPath of tree.files) {
        if (!known.has(relPath)) {
            fail(
                relPath,
                'is in src-tauri/src/constants but the generator has no instruction for it',
                'add it to EXPORTED_FILES in scripts/generate-ts-constants.js, or to BACKEND_ONLY_FILES with the reason it stays in Rust.',
            );
        }
    }
    for (const relPath of known) {
        if (!tree.files.includes(relPath)) {
            fail(relPath, 'is named in the generator but no longer exists', 'remove it from the generator.');
        }
    }

    // Rule 2: every constant in the tree is read, exported or not.
    for (const module of tree.modules.values()) {
        for (const records of module.consts.values()) {
            for (const record of records) {
                if (record.cfg === 'test') continue;
                evaluateRecord(tree, record);
            }
        }
    }

    const constants = {};
    for (const [exportName, relPath] of Object.entries(EXPORTED_FILES)) {
        constants[exportName] = flattenFile(tree, relPath);
        if (Object.keys(constants[exportName]).length === 0) {
            fail(relPath, `produced no constants for the '${exportName}' export`, 'an empty export means the frontend reads nothing; something is wrong upstream.');
        }
    }
    return constants;
}

// ---------------------------------------------------------------------------
// Emission
// ---------------------------------------------------------------------------

/** A TypeScript single-quoted string literal that means exactly `value`. */
function tsString(value) {
    const escaped = value
        .replace(/\\/g, '\\\\')
        .replace(/'/g, "\\'")
        .replace(/\n/g, '\\n')
        .replace(/\r/g, '\\r')
        .replace(/\t/g, '\\t')
        // eslint-disable-next-line no-control-regex
        .replace(/[\u0000-\u0008\u000b\u000c\u000e-\u001f\u007f]/g, (ch) =>
            `\\x${ch.charCodeAt(0).toString(16).padStart(2, '0')}`);
    return `'${escaped}'`;
}

function formatValue(value) {
    if (Array.isArray(value)) return `[${value.map(formatValue).join(', ')}]`;
    if (typeof value === 'string') return tsString(value);
    if (typeof value === 'number' || typeof value === 'boolean') return String(value);
    if (isPlatformValue(value)) return `isMac ? ${formatValue(value.macos)} : ${formatValue(value.other)}`;
    throw new ConstantsError(`the generator has no TypeScript form for ${JSON.stringify(value)}`);
}

function block(entries) {
    return entries.map(([name, value]) => `  ${name}: ${formatValue(value)},`).join('\n');
}

function entriesOf(map, predicate) {
    return Object.entries(map).filter(([name]) => (predicate ? predicate(name) : true));
}

function stripPrefix(entries, prefix) {
    return entries.map(([name, value]) => [name.slice(prefix.length), value]);
}

/**
 * The keyboard shortcuts, by name. `settings::defaults` mixes shortcut
 * defaults in with every other default, so the export cannot be "whatever is
 * in defaults" - that is how PERMISSION_MODE_* and MOUSE_CONTROL_ALWAYS ended
 * up filed as keyboard shortcuts. The list is checked against Rust in both
 * directions below, so it cannot drift.
 */
const KEYBOARD_SHORTCUT_NAMES = [
    'AGENT_MODE',
    'DICTATION_INPUT',
    'STOP_CURRENT_TASK',
    'OPEN_SETTINGS',
];

/** What a shortcut value looks like: modifiers joined to a key by '+'. */
const SHORTCUT_SHAPED =
    /^(?:(?:Cmd|Command|Ctrl|Control|Alt|Option|Shift|Super|Meta)\+)*(?:[A-Z]|F\d{1,2}|Space|Escape|Enter|Return|Tab|Backspace|Delete|Comma|Period|Slash|Backslash|Semicolon|Quote|Minus|Equal|Plus|Up|Down|Left|Right|Home|End|PageUp|PageDown|Grave|BracketLeft|BracketRight)$/;

function splitSettingsDefaults(settings) {
    const defaults = stripPrefix(entriesOf(settings, (name) => name.startsWith('DEFAULTS_')), 'DEFAULTS_');
    const byName = new Map(defaults);

    for (const name of KEYBOARD_SHORTCUT_NAMES) {
        if (!byName.has(name)) {
            fail(
                'scripts/generate-ts-constants.js',
                `KEYBOARD_SHORTCUT_NAMES lists '${name}' but settings::defaults has no such constant`,
                'remove it from the list, or restore the Rust constant.',
            );
        }
    }

    const shortcuts = [];
    const rest = [];
    for (const [name, value] of defaults) {
        const isListed = KEYBOARD_SHORTCUT_NAMES.includes(name);
        const candidates = isPlatformValue(value) ? [value.macos, value.other] : [value];
        const looksLikeShortcut = candidates.every((v) => typeof v === 'string' && SHORTCUT_SHAPED.test(v));
        if (looksLikeShortcut && !isListed) {
            fail(
                'src-tauri/src/constants/settings.rs',
                `defaults::${name} = ${JSON.stringify(candidates[0])} is shaped like a keyboard shortcut but is not in KEYBOARD_SHORTCUT_NAMES`,
                'add it to KEYBOARD_SHORTCUT_NAMES in scripts/generate-ts-constants.js so it reaches KEYBOARD_SHORTCUTS, or rename it.',
            );
        }
        (isListed ? shortcuts : rest).push([name, value]);
    }

    return { shortcuts, rest };
}

/**
 * Generate the TypeScript constants file.
 */
export function generateTypeScript(constants) {
    const { shortcuts, settingDefaults } = (() => {
        const split = splitSettingsDefaults(constants.settings || {});
        return { shortcuts: split.shortcuts, settingDefaults: split.rest };
    })();

    const computerActions = (() => {
        const actions = {};
        for (const [name, value] of Object.entries(constants.agent)) {
            const isAction =
                name.startsWith('COMPUTER_ACTIONS_') ||
                name.startsWith('TOOL_NAMES_ACTION_') ||
                (name.startsWith('ACTION_') && !name.startsWith('TOOL_NAMES_'));
            if (!isAction) continue;
            let cleanName = name;
            if (cleanName.startsWith('COMPUTER_ACTIONS_')) cleanName = cleanName.replace('COMPUTER_ACTIONS_', '');
            else if (cleanName.startsWith('TOOL_NAMES_ACTION_')) cleanName = cleanName.replace('TOOL_NAMES_ACTION_', '');
            else cleanName = cleanName.replace('ACTION_', '');
            if (!(cleanName in actions) || name.startsWith('ACTION_') || name.startsWith('TOOL_NAMES_ACTION_')) {
                actions[cleanName] = value;
            }
        }
        return Object.entries(actions);
    })();

    return `// Generated file - do not edit manually
// This file is auto-generated from Rust constants
// Run 'bun run generate-constants' to update

// Platform detection, used by any constant Rust defines per platform.
const isMac = typeof navigator !== 'undefined' && navigator.platform.toLowerCase().includes('mac');

export const EVENTS = {
${block(entriesOf(constants.events))}
} as const;

export const TIMEOUTS = {
${block(entriesOf(constants.timeouts))}
} as const;

export const PORTS = {
${block(entriesOf(constants.ports))}
} as const;

export const API_ENDPOINTS = {
${block(entriesOf(constants.api))}
} as const;

export const APP_IDENTITY = {
${block(entriesOf(constants.app))}
} as const;

export const UI = {
${block(entriesOf(constants.ui))}
} as const;

export const AUDIO = {
${block(entriesOf(constants.audio))}
} as const;

export const COMMANDS = {
${block(entriesOf(constants.commands))}
} as const;

export const MEMORY = {
${block(entriesOf(constants.memory))}
} as const;

export const AGENT = {
${block(entriesOf(constants.agent))}
} as const;

// Keyboard shortcuts with platform-specific defaults
export const KEYBOARD_SHORTCUTS = {
${block(shortcuts)}
} as const;

// Everything else settings::defaults holds, with Rust's types kept: a Rust
// bool arrives as a boolean, not the string 'true'.
export const SETTING_DEFAULTS = {
${block(settingDefaults)}
} as const;

export const SETTINGS = {
${block(entriesOf(constants.settings, (name) => !name.startsWith('DEFAULTS_')))}
} as const;

export const COMPUTER_ACTIONS = {
${block(computerActions)}
} as const;

export const TOOL_NAMES = {
${block(stripPrefix(entriesOf(constants.agent, (name) => name.startsWith('TOOL_NAMES_')), 'TOOL_NAMES_'))}
} as const;

export const FILE_EXTENSIONS = {
${block(entriesOf(constants.files))}
} as const;

export const PERMISSION_TYPES = {
${block(stripPrefix(entriesOf(constants.permissions, (name) => name.startsWith('TYPES_')), 'TYPES_'))}
} as const;

export const CHROME_DEBUG = {
${block(stripPrefix(entriesOf(constants.ports, (name) => name.startsWith('CHROME_DEBUG_PORT_')), 'CHROME_DEBUG_PORT_'))}
} as const;

export const WINDOW_LABELS = {
${block(stripPrefix(entriesOf(constants.ui, (name) => name.startsWith('WINDOW_LABELS_')), 'WINDOW_LABELS_'))}
} as const;

// Frontend-specific constants (not duplicated from Rust)
export const CSS_CLASSES = {
  HIDDEN: 'hidden',
  VISIBLE: 'visible',
  LOADING: 'loading',
  ERROR: 'error',
  SUCCESS: 'success',
  FADE_IN: 'fade-in',
  FADE_OUT: 'fade-out',
  SLIDE_IN: 'slide-in',
  SLIDE_OUT: 'slide-out',
} as const;

export const LOCAL_STORAGE_KEYS = {
  USER_PREFERENCES: 'juno_user_preferences',
  CHAT_HISTORY: 'juno_chat_history',
  CLOUD_CONFIG: 'juno_cloud_config',
  VOICE_SETTINGS: 'juno_voice_settings',
  THEME_PREFERENCE: 'juno_theme',
  WINDOW_STATE: 'juno_window_state',
  // Developer-tools overlay toggles, read by the overlays and written by the
  // Visualization settings panel. Per-window display prefs, not settings.
  SHOW_KEY_PRESS_OVERLAY: 'juno-show-key-press-overlay',
  SHOW_COMMAND_OVERLAY: 'juno-show-command-overlay',
  SHOW_CLICK_VISUALIZATION: 'juno-show-click-visualization',
  SHOW_DESKTOP_CURSOR_VISUALIZATION: 'juno-show-desktop-cursor-visualization',
} as const;

export const REGEX_PATTERNS = {
  EMAIL: /^[^\\s@]+@[^\\s@]+\\.[^\\s@]+$/,
  URL: /^https?:\\/\\/.+/,
  JSON: /^[\\s]*[{\\[]/,
  WHITESPACE_ONLY: /^\\s*$/,
  WAKE_WORD: /^[a-zA-Z\\s]{2,20}$/,
  COMMAND_PREFIX: /^[\\/!@#]/,
} as const;

export const LIMITS = {
  MAX_MESSAGE_LENGTH: 10000,
  MAX_FILENAME_LENGTH: 255,
  MAX_CHAT_HISTORY_ITEMS: 1000,
  MAX_WAKE_WORDS: 10,
  MAX_RECENT_FILES: 20,
  MAX_SEARCH_RESULTS: 100,
  MIN_WINDOW_WIDTH: 320,
  MIN_WINDOW_HEIGHT: 240,
} as const;

export const HTTP_STATUS = {
  OK: 200,
  CREATED: 201,
  BAD_REQUEST: 400,
  UNAUTHORIZED: 401,
  FORBIDDEN: 403,
  NOT_FOUND: 404,
  INTERNAL_SERVER_ERROR: 500,
} as const;

export const ERROR_MESSAGES = {
  UNKNOWN_ERROR: 'An unknown error occurred',
  NETWORK_ERROR: 'Network connection error',
  TIMEOUT_ERROR: 'Request timed out',
  PERMISSION_DENIED: 'Permission denied',
  VOICE_UNAVAILABLE: 'Voice transcription unavailable',
  AGENT_BUSY: 'Agent is currently processing another request',
  INVALID_COMMAND: 'Invalid command or parameters',
  CLOUD_DISCONNECTED: 'Cloud service disconnected',
} as const;

export const SUCCESS_MESSAGES = {
  SAVE_SUCCESS: 'Successfully saved',
  UPLOAD_SUCCESS: 'Upload completed',
  CONNECTION_SUCCESS: 'Connected successfully',
  SYNC_SUCCESS: 'Synchronized successfully',
} as const;

export const DEFAULT_CONFIG = {
  theme: 'system',
  language: 'en',
  autoSave: true,
  soundEnabled: true,
  voiceSensitivity: 0.5,
  wakeWords: ['hey juno', 'computer'],
  cloudEnabled: false,
  debugMode: false,
} as const;

// Type helpers
export type EventName = typeof EVENTS[keyof typeof EVENTS];
export type WindowLabel = typeof WINDOW_LABELS[keyof typeof WINDOW_LABELS];
export type ApiEndpoint = typeof API_ENDPOINTS[keyof typeof API_ENDPOINTS];
export type FileExtension = typeof FILE_EXTENSIONS[keyof typeof FILE_EXTENSIONS];
export type PermissionType = typeof PERMISSION_TYPES[keyof typeof PERMISSION_TYPES];
export type ChromeDebugPort = typeof CHROME_DEBUG[keyof typeof CHROME_DEBUG];
export type CommandName = typeof COMMANDS[keyof typeof COMMANDS];
export type ComputerAction = typeof COMPUTER_ACTIONS[keyof typeof COMPUTER_ACTIONS];
export type ToolName = typeof TOOL_NAMES[keyof typeof TOOL_NAMES];
export type SettingDefault = typeof SETTING_DEFAULTS[keyof typeof SETTING_DEFAULTS];
export type DefaultConfig = typeof DEFAULT_CONFIG;
`;
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

function main() {
    try {
        console.log('Generating TypeScript constants from Rust...');

        const constants = parseRustConstants();
        const typescript = generateTypeScript(constants);
        fs.writeFileSync(TS_OUTPUT_FILE, typescript);

        console.log(`Generated ${path.relative(REPO_ROOT, TS_OUTPUT_FILE)}`);
        for (const name of Object.keys(constants)) {
            console.log(`   - ${name}: ${Object.keys(constants[name]).length}`);
        }
    } catch (error) {
        if (error instanceof ConstantsError) {
            console.error('\nThe constants generator refused to guess:\n');
            console.error(error.message);
            console.error('\nNothing was written. src/lib/constants.generated.ts is unchanged.\n');
        } else {
            console.error('Error generating constants:', error);
        }
        process.exit(1);
    }
}

if (import.meta.url === `file://${process.argv[1]}`) {
    main();
}
