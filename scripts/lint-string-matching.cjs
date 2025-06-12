#!/usr/bin/env node

/**
 * String Matching Anti-Pattern Linter
 * 
 * Scans the codebase for string matching anti-patterns and suggests
 * structured alternatives based on the anti-string-matching rules.
 */

const fs = require('fs');
const path = require('path');
const glob = require('glob');

class StringMatchingLinter {
  constructor() {
    this.violations = [];
    this.stats = {
      filesScanned: 0,
      violationsFound: 0,
      highSeverity: 0,
      mediumSeverity: 0,
      lowSeverity: 0
    };
  }

  // Anti-pattern detection rules
  static RULES = {
    CHAINED_REPLACE: {
      name: 'Chained Replace Operations',
      severity: 'high',
      pattern: /\.replace\([^)]+\)\s*\.replace\([^)]+\)\s*\.replace/g,
      description: 'Found 3+ chained .replace() calls - use data-driven transformation',
      suggestion: 'Use configuration array with { pattern, replacement } objects'
    },

    MULTIPLE_CONTAINS: {
      name: 'Multiple Contains Checks',
      severity: 'high', 
      pattern: /\.contains\([^)]+\)\s*\|\|\s*\w+\.contains\([^)]+\)\s*\|\|\s*\w+\.contains\([^)]+\)\s*\|\|/g,
      description: 'Found 4+ .contains() checks in condition - use Set or configuration',
      suggestion: 'Use HashSet or static configuration for membership testing'
    },

    STRING_EQUALITY_CHAIN: {
      name: 'String Equality Classification',
      severity: 'medium',
      pattern: /===?\s*["'][^"']+["']\s*\|\|\s*\w+\s*===?\s*["'][^"']+["']\s*\|\|\s*\w+\s*===?\s*["'][^"']+["']\s*\|\|/g,
      description: 'Found 4+ string equality checks - use enum or map',
      suggestion: 'Use enum or Map for type classification instead of string comparison'
    },

    HARDCODED_STRING_LIST: {
      name: 'Hardcoded String Array',
      severity: 'medium',
      pattern: /\[\s*["'][^"']+["']\s*,\s*["'][^"']+["']\s*,\s*["'][^"']+["']\s*,\s*["'][^"']+["']/g,
      description: 'Found hardcoded string array with 4+ items - consider external configuration',
      suggestion: 'Move to configuration file or constants module'
    },

    REPLACE_WITH_EMPTY: {
      name: 'Multiple Empty Replacements',
      severity: 'low',
      pattern: /\.replace\([^)]*,\s*["']["']\)\s*\.replace\([^)]*,\s*["']["']\)/g,
      description: 'Found multiple .replace() with empty strings - use removal configuration',
      suggestion: 'Use array of patterns to remove instead of chained replacements'
    },

    MANUAL_CASE_CONVERSION: {
      name: 'Manual Case Conversion',
      severity: 'low',
      pattern: /\.replace\([^)]*[A-Z][^)]*,.*\)/g,
      description: 'Possible manual case conversion - use proper string transformation',
      suggestion: 'Use built-in case conversion methods or regex with proper capture groups'
    }
  };

  /**
   * Scan a file for string matching anti-patterns
   */
  scanFile(filePath) {
    try {
      const content = fs.readFileSync(filePath, 'utf8');
      this.stats.filesScanned++;

      for (const [ruleId, rule] of Object.entries(StringMatchingLinter.RULES)) {
        const matches = [...content.matchAll(rule.pattern)];
        
        for (const match of matches) {
          const lineNumber = this.getLineNumber(content, match.index);
          const context = this.getContext(content, match.index, 100);
          
          this.violations.push({
            file: filePath,
            rule: ruleId,
            severity: rule.severity,
            line: lineNumber,
            description: rule.description,
            suggestion: rule.suggestion,
            context: context.trim(),
            match: match[0]
          });

          this.stats.violationsFound++;
          this.stats[rule.severity + 'Severity']++;
        }
      }
    } catch (error) {
      console.warn(`⚠️  Could not scan ${filePath}: ${error.message}`);
    }
  }

  /**
   * Get line number for a character index
   */
  getLineNumber(content, index) {
    return content.substring(0, index).split('\n').length;
  }

  /**
   * Get context around a match
   */
  getContext(content, index, radius = 50) {
    const start = Math.max(0, index - radius);
    const end = Math.min(content.length, index + radius);
    return content.substring(start, end);
  }

  /**
   * Scan multiple files based on glob patterns
   */
  scanFiles(patterns) {
    const allFiles = new Set();
    
    for (const pattern of patterns) {
      const files = glob.sync(pattern, { 
        ignore: ['**/node_modules/**', '**/target/**', '**/dist/**', '**/.git/**']
      });
      files.forEach(file => allFiles.add(file));
    }

    console.log(`🔍 Scanning ${allFiles.size} files for string matching anti-patterns...\n`);

    for (const file of allFiles) {
      this.scanFile(file);
    }
  }

  /**
   * Generate refactoring suggestions for common patterns
   */
  generateRefactoringSuggestions(violation) {
    const suggestions = [];

    switch (violation.rule) {
      case 'CHAINED_REPLACE':
        suggestions.push({
          title: 'Data-Driven Transformation',
          code: `
// ✅ GOOD: Configuration-driven approach
const TRANSFORMATIONS = [
  { pattern: /Card:/g, replacement: "" },
  { pattern: /px/g, replacement: " pixels" },
  { pattern: /×/g, replacement: " by " }
];

function transform(text) {
  return TRANSFORMATIONS.reduce((result, rule) => 
    result.replace(rule.pattern, rule.replacement), text);
}
          `.trim()
        });
        break;

      case 'MULTIPLE_CONTAINS':
        suggestions.push({
          title: 'Set-Based Classification',
          code: `
// ✅ GOOD: Set-based membership testing
const BROWSER_NAMES = new Set([
  'chrome', 'safari', 'firefox', 'edge', 'brave'
]);

function isBrowser(appName) {
  const normalized = appName.toLowerCase();
  return Array.from(BROWSER_NAMES).some(browser => 
    normalized.includes(browser));
}
          `.trim()
        });
        break;

      case 'STRING_EQUALITY_CHAIN':
        suggestions.push({
          title: 'Enum-Based Classification',
          code: `
// ✅ GOOD: Enum-based type safety
enum MessageRole {
  Thinking = "thinking",
  ToolCall = "tool_call",
  Result = "result"
}

function handleMessage(msg) {
  switch (msg.role as MessageRole) {
    case MessageRole.Thinking:
      return handleThinking(msg);
    case MessageRole.ToolCall:
      return handleToolCall(msg);
    case MessageRole.Result:
      return handleResult(msg);
    default:
      throw new Error(\`Unknown role: \${msg.role}\`);
  }
}
          `.trim()
        });
        break;
    }

    return suggestions;
  }

  /**
   * Generate detailed report
   */
  generateReport() {
    console.log('\n📊 String Matching Anti-Pattern Analysis Report\n');
    console.log('=' .repeat(60));
    
    // Summary
    console.log(`📈 SUMMARY:`);
    console.log(`   Files Scanned: ${this.stats.filesScanned}`);
    console.log(`   Total Violations: ${this.stats.violationsFound}`);
    console.log(`   🔴 High Severity: ${this.stats.highSeverity}`);
    console.log(`   🟡 Medium Severity: ${this.stats.mediumSeverity}`);
    console.log(`   🟢 Low Severity: ${this.stats.lowSeverity}\n`);

    if (this.violations.length === 0) {
      console.log('✅ No string matching anti-patterns found! Great job!');
      return;
    }

    // Group violations by severity
    const grouped = this.groupViolationsBySeverity();

    for (const [severity, violations] of Object.entries(grouped)) {
      if (violations.length === 0) continue;

      const icon = severity === 'high' ? '🔴' : severity === 'medium' ? '🟡' : '🟢';
      console.log(`${icon} ${severity.toUpperCase()} SEVERITY (${violations.length} issues)`);
      console.log('-'.repeat(40));

      for (const violation of violations.slice(0, 5)) { // Show first 5 per severity
        console.log(`📁 ${violation.file}:${violation.line}`);
        console.log(`   ${violation.description}`);
        console.log(`   💡 ${violation.suggestion}`);
        console.log(`   📝 Code: ${violation.match.substring(0, 80)}...`);
        console.log('');
      }

      if (violations.length > 5) {
        console.log(`   ... and ${violations.length - 5} more ${severity} severity issues\n`);
      }
    }

    // Refactoring examples for worst violations
    const highSeverity = grouped.high || [];
    if (highSeverity.length > 0) {
      console.log('\n🔧 REFACTORING SUGGESTIONS\n');
      console.log('=' .repeat(60));
      
      const example = highSeverity[0];
      const suggestions = this.generateRefactoringSuggestions(example);
      
      for (const suggestion of suggestions) {
        console.log(`💡 ${suggestion.title}:`);
        console.log(suggestion.code);
        console.log('');
      }
    }

    // Summary recommendations
    console.log('\n📋 RECOMMENDED ACTIONS\n');
    console.log('=' .repeat(60));
    console.log('1. 🚨 Fix HIGH severity violations first (these are blocking)');
    console.log('2. 📚 Review docs/rules/anti-string-matching.md for patterns');
    console.log('3. 🔧 Use configuration-driven approaches for transformations');
    console.log('4. 🧪 Add tests for individual transformation rules');
    console.log('5. 🔄 Run this linter in CI/CD to prevent regressions');
    
    if (this.stats.highSeverity > 0) {
      console.log('\n❌ Build should FAIL due to high severity violations');
      process.exit(1);
    } else if (this.stats.mediumSeverity > 5) {
      console.log('\n⚠️  Build should WARN due to multiple medium severity violations');
      process.exit(0);
    } else {
      console.log('\n✅ No critical string matching anti-patterns found');
      process.exit(0);
    }
  }

  /**
   * Group violations by severity
   */
  groupViolationsBySeverity() {
    return this.violations.reduce((groups, violation) => {
      if (!groups[violation.severity]) {
        groups[violation.severity] = [];
      }
      groups[violation.severity].push(violation);
      return groups;
    }, {});
  }
}

// CLI execution
if (require.main === module) {
  const linter = new StringMatchingLinter();
  
  // Default patterns to scan
  const patterns = [
    'src/**/*.{js,ts,jsx,tsx}',
    'src-tauri/src/**/*.rs',
    'tauri-plugin-voice-transcription/src/**/*.rs',
    'backend-server/src/**/*.js'
  ];

  linter.scanFiles(patterns);
  linter.generateReport();
}

module.exports = StringMatchingLinter;