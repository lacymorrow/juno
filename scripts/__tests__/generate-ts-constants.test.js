/**
 * Tests for the Rust -> TypeScript constants generator.
 *
 * Two jobs, matching the generator's two rules:
 *
 *   1. Over the real `src-tauri/src/constants` tree: every `pub const` that
 *      reaches the frontend reaches it with the value Rust holds, and no
 *      constant is quietly missing.
 *   2. Over synthetic source: every value form either parses to the right
 *      value or raises. A silent skip is a failure here, not a warning.
 */
import fs from 'fs';
import path from 'path';
import { describe, expect, it } from 'vitest';
import {
    BACKEND_ONLY_FILES,
    ConstantsError,
    EXPORTED_FILES,
    generateTypeScript,
    parseRustConstants,
    parseSources,
} from '../generate-ts-constants.js';

const CONSTANTS_DIR = path.resolve(__dirname, '../../src-tauri/src/constants');

const readRust = (relPath) => fs.readFileSync(path.join(CONSTANTS_DIR, relPath), 'utf8');

function rustFiles(dir = CONSTANTS_DIR, base = '') {
    return fs.readdirSync(dir, { withFileTypes: true }).flatMap((entry) => {
        const rel = base ? `${base}/${entry.name}` : entry.name;
        if (entry.isDirectory()) return rustFiles(path.join(dir, entry.name), rel);
        return entry.name.endsWith('.rs') ? [rel] : [];
    });
}

/** Every `pub const NAME` in a file, found without the generator's help. */
function declaredNames(source) {
    const withoutTests = source.split(/#\[cfg\(test\)\]/)[0];
    return [...withoutTests.matchAll(/pub const (\w+)\s*:/g)].map((match) => match[1]);
}

const UNESCAPE = { n: '\n', r: '\r', t: '\t', 0: '\0', '\\': '\\', '"': '"', "'": "'" };

/** Every double-quoted literal anywhere in the tree, as a set. */
function everyStringLiteralInTree() {
    const literals = new Set();
    for (const relPath of rustFiles()) {
        for (const match of readRust(relPath).matchAll(/"((?:[^"\\]|\\.)*)"/g)) {
            literals.add(match[1]);
            literals.add(match[1].replace(/\\(.)/g, (whole, ch) => (ch in UNESCAPE ? UNESCAPE[ch] : whole)));
        }
    }
    return literals;
}

const constants = parseRustConstants();

describe('the real constants tree', () => {
    it('parses without a single warning or skip', () => {
        expect(Object.keys(constants).length).toBe(Object.keys(EXPORTED_FILES).length);
        for (const name of Object.keys(EXPORTED_FILES)) {
            expect(Object.keys(constants[name]).length).toBeGreaterThan(0);
        }
    });

    it('accounts for every .rs file: exported, or backend-only with a reason', () => {
        const exported = new Set(Object.values(EXPORTED_FILES));
        const backend = new Set(Object.keys(BACKEND_ONLY_FILES));
        for (const relPath of rustFiles()) {
            expect(exported.has(relPath) || backend.has(relPath), `${relPath} is unaccounted for`).toBe(true);
            expect(exported.has(relPath) && backend.has(relPath), `${relPath} is in both lists`).toBe(false);
        }
        for (const reason of Object.values(BACKEND_ONLY_FILES)) {
            expect(reason.length).toBeGreaterThan(10);
        }
    });

    it('drops no constant from any exported file', () => {
        const missing = [];
        for (const [exportName, relPath] of Object.entries(EXPORTED_FILES)) {
            const flat = constants[exportName];
            const keys = Object.keys(flat);
            for (const name of declaredNames(readRust(relPath))) {
                const found = keys.some((flatKey) => flatKey === name || flatKey.endsWith(`_${name}`));
                if (!found) missing.push(`${relPath}: ${name}`);
            }
        }
        expect(missing).toEqual([]);
    });

    it('invents no string: every string value is a literal somewhere in the tree', () => {
        const literals = everyStringLiteralInTree();
        const invented = [];
        const check = (where, value) => {
            if (Array.isArray(value)) { value.forEach((item) => check(where, item)); return; }
            if (value && typeof value === 'object') {
                check(where, value.macos);
                check(where, value.other);
                return;
            }
            if (typeof value !== 'string') return;
            if (!literals.has(value)) invented.push(`${where} = ${JSON.stringify(value)}`);
        };
        for (const [exportName, flat] of Object.entries(constants)) {
            for (const [name, value] of Object.entries(flat)) check(`${exportName}.${name}`, value);
        }
        expect(invented).toEqual([]);
    });

    it('keeps the values that used to be dropped, and keeps them right', () => {
        expect(constants.api.TOOL_VERSION_GROUPS_COMPUTER_USE_2025_11_24_TOOLS).toEqual([
            'computer_20251124', 'text_editor_20250728', 'bash_20250124',
        ]);
        expect(constants.ui.STANDARD_RESOLUTIONS_HD_1080).toEqual([1920, 1080]);
        expect(constants.ui.STANDARD_RESOLUTIONS_LEGACY_RESOLUTIONS).toEqual([
            [1024, 768], [1280, 800], [1366, 768],
        ]);
        expect(constants.memory.COMMON_WORDS[0]).toBe('the');
        expect(constants.settings.DEFAULTS_CLAUDE_CLI_EFFORT_LEVELS).toEqual([
            'low', 'medium', 'high', 'xhigh', 'max',
        ]);
        expect(constants.files.EXTENSIONS_PRODUCTION_EXTENSIONS).toContain('scala');
    });

    it('reads a Rust bool as a boolean, not the string "true"', () => {
        expect(constants.settings.DEFAULTS_SOUND_ENABLED).toBe(true);
        expect(constants.settings.DEFAULTS_CLOUD_ENABLED).toBe(false);
    });

    it('carries a macOS / not-macOS pair as both values', () => {
        expect(constants.settings.DEFAULTS_AGENT_MODE).toEqual({ macos: 'Option+D', other: 'Alt+D' });
    });

    it('round-trips escapes', () => {
        expect(constants.files.LINE_ENDINGS_DEFAULT_LF).toBe('\n');
        expect(constants.files.PATH_PATTERNS_PATH_TRAVERSAL_WINDOWS).toBe('..\\');
    });
});

describe('the generated TypeScript', () => {
    const output = generateTypeScript(constants);

    it('files keyboard shortcuts as keyboard shortcuts and nothing else', () => {
        const block = /export const KEYBOARD_SHORTCUTS = \{([\s\S]*?)\n\} as const;/.exec(output)[1];
        const names = [...block.matchAll(/^ {2}(\w+):/gm)].map((match) => match[1]);
        expect(names).toEqual(['AGENT_MODE', 'DICTATION_INPUT', 'STOP_CURRENT_TASK', 'OPEN_SETTINGS']);
        expect(block).not.toMatch(/PERMISSION_MODE/);
        expect(block).not.toMatch(/MOUSE_CONTROL_ALWAYS/);
    });

    it('puts the other settings defaults in SETTING_DEFAULTS with Rust types', () => {
        const block = /export const SETTING_DEFAULTS = \{([\s\S]*?)\n\} as const;/.exec(output)[1];
        expect(block).toMatch(/^ {2}SOUND_ENABLED: true,$/m);
        expect(block).toMatch(/^ {2}PERMISSION_MODE: 'ask_when_risky',$/m);
        expect(block).toMatch(/^ {2}CLAUDE_CLI_EFFORT_LEVELS: \['low', 'medium', 'high', 'xhigh', 'max'\],$/m);
        expect(block).not.toMatch(/'true'/);
    });

    it('declares isMac before any export that uses it', () => {
        expect(output.indexOf('const isMac =')).toBeLessThan(output.indexOf('export const EVENTS'));
    });

    it('refuses a shortcut-shaped default that is not filed as a shortcut', () => {
        const drifted = {
            ...constants,
            settings: { ...constants.settings, DEFAULTS_SOME_NEW_HOTKEY: 'Cmd+Shift+K' },
        };
        expect(() => generateTypeScript(drifted)).toThrow(/shaped like a keyboard shortcut/);
    });

    it('refuses a shortcut name the Rust defaults no longer have', () => {
        const settings = { ...constants.settings };
        delete settings.DEFAULTS_OPEN_SETTINGS;
        expect(() => generateTypeScript({ ...constants, settings })).toThrow(/no such constant/);
    });
});

// ---------------------------------------------------------------------------
// The value grammar, form by form. Each either parses to the right value or
// raises; nothing is allowed to pass quietly with the wrong answer.
// ---------------------------------------------------------------------------

const one = (body) => parseSources(`pub mod m {\n${body}\n}\n`);

describe('values the generator reads', () => {
    it('reads strings, including escapes and a URL that holds //', () => {
        const flat = one(`
            pub const PLAIN: &str = "hello";
            pub const QUOTED: &str = "say \\"hi\\"";
            pub const NEWLINE: &str = "\\n";
            pub const BACKSLASH: &str = "..\\\\";
            pub const URL: &str = "https://junebug.ai/path";
            pub const TAB: &str = "a\\tb";
        `);
        expect(flat.M_PLAIN).toBe('hello');
        expect(flat.M_QUOTED).toBe('say "hi"');
        expect(flat.M_NEWLINE).toBe('\n');
        expect(flat.M_BACKSLASH).toBe('..\\');
        expect(flat.M_URL).toBe('https://junebug.ai/path');
        expect(flat.M_TAB).toBe('a\tb');
    });

    it('reads booleans as booleans', () => {
        const flat = one('pub const YES: bool = true;\npub const NO: bool = false;');
        expect(flat.M_YES).toBe(true);
        expect(flat.M_NO).toBe(false);
    });

    it('reads numbers: suffixes, separators, negatives, floats, hex, binary, octal', () => {
        const flat = one(`
            pub const SUFFIXED: u64 = 100u64;
            pub const SEPARATED: u32 = 1_000;
            pub const BOTH: u64 = 30_000u64;
            pub const NEGATIVE: i32 = -32600;
            pub const FLOATY: f32 = 0.5;
            pub const FLOAT_SUFFIX: f64 = 1.5f64;
            pub const HEX: u64 = 0x00100000;
            pub const SMALL_HEX: u64 = 0x01;
            pub const BINARY: u8 = 0b1010;
            pub const OCTAL: u16 = 0o17;
        `);
        expect(flat.M_SUFFIXED).toBe(100);
        expect(flat.M_SEPARATED).toBe(1000);
        expect(flat.M_BOTH).toBe(30000);
        expect(flat.M_NEGATIVE).toBe(-32600);
        expect(flat.M_FLOATY).toBe(0.5);
        expect(flat.M_FLOAT_SUFFIX).toBe(1.5);
        expect(flat.M_HEX).toBe(1048576);
        expect(flat.M_SMALL_HEX).toBe(1);
        expect(flat.M_BINARY).toBe(10);
        expect(flat.M_OCTAL).toBe(15);
    });

    it('reads arrays and tuples, on one line or many', () => {
        const flat = one(`
            pub const SLICE: &[&str] = &["a", "b"];
            pub const SIZED: [&str; 2] = ["a", "b"];
            pub const WRAPPED: &[&str] = &[
                "a", "b",
                "c",
            ];
            pub const PAIR: (u32, u32) = (1024, 768);
            pub const PAIRS: [(u32, u32); 2] = [(1, 2), (3, 4)];
            pub const EMPTY: &[&str] = &[];
        `);
        expect(flat.M_SLICE).toEqual(['a', 'b']);
        expect(flat.M_SIZED).toEqual(['a', 'b']);
        expect(flat.M_WRAPPED).toEqual(['a', 'b', 'c']);
        expect(flat.M_PAIR).toEqual([1024, 768]);
        expect(flat.M_PAIRS).toEqual([[1, 2], [3, 4]]);
        expect(flat.M_EMPTY).toEqual([]);
    });

    it('reads a value that spans lines and one with a trailing comment', () => {
        const flat = one(`
            pub const WRAPPED: &str =
                "request_screen_recording_permission_native";
            pub const COMMENTED: u64 = 300; // 5 minutes
            pub const BLOCK: u64 = /* inline */ 7;
        `);
        expect(flat.M_WRAPPED).toBe('request_screen_recording_permission_native');
        expect(flat.M_COMMENTED).toBe(300);
        expect(flat.M_BLOCK).toBe(7);
    });

    it('is not fooled by a brace or a // inside a string', () => {
        const flat = parseSources(`
            pub mod templates {
                pub const OPEN_BRACE: &str = "a {";
                pub const FORMAT: &str = "File '{}' not allowed";
                pub const SLASHES: &str = "http://x";
            }
            pub mod after {
                pub const STILL_HERE: &str = "yes";
            }
        `);
        expect(flat.TEMPLATES_OPEN_BRACE).toBe('a {');
        expect(flat.TEMPLATES_FORMAT).toBe("File '{}' not allowed");
        expect(flat.AFTER_STILL_HERE).toBe('yes');
    });

    it('folds arithmetic with the declared type: integer division truncates', () => {
        const flat = one(`
            pub const FPS: u64 = 60;
            pub const FRAME_MS: u64 = 1000 / FPS;
            pub const BYTES: usize = 100 * 1024 * 1024;
            pub const RATIO: f64 = 1000.0 / 8.0;
            pub const SUM: u32 = 2 + 3 * 4;
        `);
        expect(flat.M_FRAME_MS).toBe(16);
        expect(flat.M_BYTES).toBe(104857600);
        expect(flat.M_RATIO).toBe(125);
        expect(flat.M_SUM).toBe(14);
    });

    it('resolves a constant defined in terms of another one', () => {
        const flat = parseSources(`
            pub mod modes {
                pub const PASTE: &str = "paste";
            }
            pub mod tools {
                pub const A: &str = "tool_a";
                pub const B: &str = "tool_b";
                pub const SET: &[&str] = &[A, B];
            }
            pub mod defaults {
                pub const INSERTION_MODE: &str = super::modes::PASTE;
                pub const ABSOLUTE: &str = crate::constants::probe::modes::PASTE;
            }
        `);
        expect(flat.TOOLS_SET).toEqual(['tool_a', 'tool_b']);
        expect(flat.DEFAULTS_INSERTION_MODE).toBe('paste');
        expect(flat.DEFAULTS_ABSOLUTE).toBe('paste');
    });

    it('resolves a reference that arrives through a glob import', () => {
        const flat = parseSources(`
            pub mod types {
                pub const COMPUTER: &str = "computer_20251124";
            }
            pub mod groups {
                use super::types::*;
                pub const TOOLS: &[&str] = &[COMPUTER];
            }
        `);
        expect(flat.GROUPS_TOOLS).toEqual(['computer_20251124']);
    });

    it('resolves a reference across files', () => {
        const flat = parseSources([
            { relPath: 'settings.rs', source: 'pub mod d { pub const E: &str = crate::constants::events::sys::CHANGED; }' },
            { relPath: 'events.rs', source: 'pub mod sys { pub const CHANGED: &str = "settings_changed"; }' },
        ]);
        expect(flat.D_E).toBe('settings_changed');
    });

    it('carries a macOS / not-macOS pair and keeps both arms', () => {
        const flat = one(`
            #[cfg(target_os = "macos")]
            pub const KEY: &str = "Cmd+Comma";
            #[cfg(not(target_os = "macos"))]
            pub const KEY: &str = "Ctrl+Comma";
        `);
        expect(flat.M_KEY).toEqual({ macos: 'Cmd+Comma', other: 'Ctrl+Comma' });
    });

    it('leaves #[cfg(test)] fixtures and private constants out', () => {
        const flat = parseSources(`
            pub mod m {
                pub const REAL: &str = "real";
                const PRIVATE: &str = "private";
            }
            #[cfg(test)]
            mod tests {
                pub const FIXTURE: &str = "fixture";
            }
        `);
        expect(flat.M_REAL).toBe('real');
        expect(Object.keys(flat)).not.toContain('M_PRIVATE');
        expect(Object.keys(flat).some((name) => name.includes('FIXTURE'))).toBe(false);
    });
});

describe('values the generator refuses', () => {
    const refuses = (body, pattern) => {
        let thrown = null;
        try {
            one(body);
        } catch (error) {
            thrown = error;
        }
        expect(thrown, `expected '${body.trim()}' to raise, and it did not`).toBeInstanceOf(ConstantsError);
        expect(thrown.message).toMatch(pattern);
        // The message has to be actionable: file, line and constant name.
        expect(thrown.message).toMatch(/probe\.rs:\d+/);
    };

    it('refuses a path out of the tree, which is the bug that started this', () => {
        // The old parser's bare `(\w+)` matched here and shipped the string
        // 'crate' to the frontend with no warning.
        refuses(
            'pub const X: &str = crate::agent::tools::SOMETHING;',
            /X is defined as 'crate::agent::tools::SOMETHING', which the generator cannot resolve/,
        );
    });

    it('refuses a reference to a constant that does not exist', () => {
        refuses('pub const X: &str = MISSING;', /X refers to 'MISSING', and the generator cannot resolve it/);
    });

    it('refuses a circular definition', () => {
        refuses('pub const A: u32 = B;\npub const B: u32 = A;', /in terms of itself/);
    });

    it('refuses a function call or a constructor', () => {
        refuses('pub const X: u64 = Duration::from_secs(5);', /calls or constructs/);
        refuses('pub const X: u64 = compute();', /calls or constructs/);
    });

    it('refuses arithmetic it cannot type', () => {
        refuses('pub const X: CGFlags = 1000 / 3;', /integer or floating-point/);
    });

    it('refuses arithmetic on a string', () => {
        refuses('pub const X: u32 = 1 + "two";', /'\+' to a number and a string/);
    });

    it('refuses a number too large for a TypeScript number', () => {
        refuses('pub const X: u64 = 18446744073709551615;', /does not survive JavaScript/);
    });

    it('refuses a #[cfg] split it does not understand', () => {
        refuses(
            '#[cfg(target_os = "windows")]\npub const X: &str = "cmd";\n#[cfg(target_os = "linux")]\npub const X: &str = "xdg-open";',
            /platform/,
        );
    });

    it('refuses two constants that would collide in the generated file', () => {
        let thrown = null;
        try {
            parseSources(`
                pub mod bar {
                    pub const STATE_IDLE: &str = "a";
                }
                pub mod bar_state {
                    pub const IDLE: &str = "b";
                }
            `);
        } catch (error) {
            thrown = error;
        }
        expect(thrown).toBeInstanceOf(ConstantsError);
        expect(thrown.message).toMatch(/both become 'BAR_STATE_IDLE'/);
    });

    it('refuses a module nested deeper than the flat keys can name', () => {
        let thrown = null;
        try {
            parseSources('pub mod outer { pub mod inner { pub const X: &str = "x"; } }');
        } catch (error) {
            thrown = error;
        }
        expect(thrown).toBeInstanceOf(ConstantsError);
        expect(thrown.message).toMatch(/nested more than one level deep/);
    });

    it('refuses a glob re-export, which could hide a missing constant', () => {
        let thrown = null;
        try {
            parseSources(`
                pub mod a { pub const X: &str = "x"; }
                pub use a::*;
            `);
        } catch (error) {
            thrown = error;
        }
        expect(thrown).toBeInstanceOf(ConstantsError);
        expect(thrown.message).toMatch(/cannot enumerate/);
    });

    it('refuses an unbalanced brace instead of ending a module early', () => {
        let thrown = null;
        try {
            parseSources('pub mod m { pub const X: &str = "x";');
        } catch (error) {
            thrown = error;
        }
        expect(thrown).toBeInstanceOf(ConstantsError);
        expect(thrown.message).toMatch(/never closed/);
    });

    it('never returns a half-parsed answer', () => {
        // One bad constant anywhere means nothing comes back, so no export can
        // silently end up empty.
        expect(() => parseSources(`
            pub mod good { pub const A: &str = "a"; }
            pub mod bad { pub const B: &str = crate::elsewhere::C; }
        `)).toThrow(ConstantsError);
    });
});
