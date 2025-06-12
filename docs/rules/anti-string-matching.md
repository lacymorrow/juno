# Anti-String Matching Rules and Guidelines

## 🚨 Core Principle
**String matching should be the LAST resort, not the first solution.**

String matching refers to solving problems through direct string manipulation (`.replace()` chains, multiple `.contains()` checks, string equality comparisons for classification, etc.) rather than using structured data approaches.

## ❌ Why String Matching is Problematic

### 1. **Brittleness**
- Small changes in input format break the entire system
- No tolerance for variations (spacing, casing, ordering)
- Hard-coded assumptions about exact string patterns

### 2. **Maintenance Nightmare**
- Adding new cases requires modifying existing code
- No clear separation between data and logic
- Difficult to understand the complete transformation rules

### 3. **Poor Testability**
- Can't test individual transformation rules in isolation
- Must test entire chains together
- Hard to identify which specific rule failed

### 4. **Performance Issues**
- Multiple string operations are inefficient
- No optimization opportunities
- Linear complexity for each operation

### 5. **Poor Extensibility**
- Can't handle conditional logic easily
- No way to add metadata or context
- Difficult to make rules configurable

## ❌ Common Anti-Patterns

### Anti-Pattern 1: Chained Replacements
```typescript
// ❌ BAD: String matching chain
speech_text = speech_text
    .replace("Card:", "")
    .replace("CardHeader:", "")
    .replace("CardTitle:", "")
    .replace("CardContent:", "")
    .replace("CardFooter:", "")
    .replace("SVG Circle", "visual circle")
    .replace("Circle:", "circle with")
    .replace("Rectangle:", "rectangle with")
    .replace("Triangle:", "triangle with")
    .replace("×", " by ")
    .replace("px", " pixels");
```

### Anti-Pattern 2: Multiple Contains Checks
```rust
// ❌ BAD: String matching for classification
if app_name.contains("chrome")
    || app_name.contains("safari")
    || app_name.contains("arc")
    || app_name.contains("firefox")
    || app_name.contains("edge")
    || app_name.contains("brave")
    || app_name.contains("opera")
    || app_name.contains("vivaldi")
    || app_name.contains("microsoft edge")
{
    // handle browser
}
```

### Anti-Pattern 3: String Equality Classification
```typescript
// ❌ BAD: String matching for type detection
if (msg.role === "thinking") {
    // handle thinking
} else if (msg.role === "tool_call_request") {
    // handle tool call
} else if (msg.role === "tool_call_result") {
    // handle result
}
```

## ✅ Better Alternatives

### Alternative 1: Data-Driven Transformations
```typescript
// ✅ GOOD: Configuration-driven approach
interface TransformationRule {
  pattern: string | RegExp;
  replacement: string;
  description?: string;
  priority?: number;
}

const UI_ELEMENT_TRANSFORMATIONS: TransformationRule[] = [
  { pattern: /Card(Header|Title|Content|Footer)?:/g, replacement: "", description: "Remove UI component prefixes" },
  { pattern: /SVG Circle/g, replacement: "visual circle", description: "Convert SVG elements" },
  { pattern: /Circle:/g, replacement: "circle with", description: "Convert shape descriptions" },
  { pattern: /Rectangle:/g, replacement: "rectangle with", description: "Convert shape descriptions" },
  { pattern: /Triangle:/g, replacement: "triangle with", description: "Convert shape descriptions" },
  { pattern: /×/g, replacement: " by ", description: "Convert multiplication symbol" },
  { pattern: /px/g, replacement: " pixels", description: "Expand pixel units" },
];

function transformSpeechText(text: string): string {
  return UI_ELEMENT_TRANSFORMATIONS.reduce((result, rule) => {
    return result.replace(rule.pattern, rule.replacement);
  }, text);
}
```

### Alternative 2: Set-Based Classification
```rust
// ✅ GOOD: Data-driven browser detection
lazy_static! {
    static ref BROWSER_IDENTIFIERS: HashSet<&'static str> = {
        [
            "chrome", "safari", "arc", "firefox", "edge", 
            "brave", "opera", "vivaldi", "microsoft edge"
        ].iter().cloned().collect()
    };
}

fn is_browser_app(app_name: &str) -> bool {
    let app_lower = app_name.to_lowercase();
    BROWSER_IDENTIFIERS.iter().any(|&browser| app_lower.contains(browser))
}
```

### Alternative 3: Enum-Based Type Safety
```typescript
// ✅ GOOD: Type-safe message handling
enum MessageRole {
  Thinking = "thinking",
  ToolCallRequest = "tool_call_request",
  ToolCallResult = "tool_call_result",
}

interface MessageHandler<T> {
  [MessageRole.Thinking]: (content: string) => T;
  [MessageRole.ToolCallRequest]: (toolName: string) => T;
  [MessageRole.ToolCallResult]: (toolName: string, result: any) => T;
}

function handleMessage<T>(msg: Message, handlers: MessageHandler<T>): T {
  switch (msg.role as MessageRole) {
    case MessageRole.Thinking:
      return handlers[MessageRole.Thinking](msg.content);
    case MessageRole.ToolCallRequest:
      return handlers[MessageRole.ToolCallRequest](msg.tool_name);
    case MessageRole.ToolCallResult:
      return handlers[MessageRole.ToolCallResult](msg.tool_name, msg.result);
    default:
      throw new Error(`Unknown message role: ${msg.role}`);
  }
}
```

### Alternative 4: Parser-Based Approach
```typescript
// ✅ GOOD: Structured parsing
interface ParsedElement {
  type: 'card' | 'shape' | 'measurement';
  subtype?: string;
  value?: string;
  unit?: string;
}

class ElementParser {
  private static PATTERNS = {
    card: /^(Card(?:Header|Title|Content|Footer)?):?\s*(.*)$/,
    shape: /^(Circle|Rectangle|Triangle):\s*(.*)$/,
    measurement: /^(\d+)\s*(px|em|rem|%)$/,
  };

  static parse(text: string): ParsedElement | null {
    for (const [type, pattern] of Object.entries(this.PATTERNS)) {
      const match = text.match(pattern);
      if (match) {
        return this.createParsedElement(type, match);
      }
    }
    return null;
  }

  private static createParsedElement(type: string, match: RegExpMatchArray): ParsedElement {
    switch (type) {
      case 'card':
        return { type: 'card', subtype: match[1], value: match[2] };
      case 'shape':
        return { type: 'shape', subtype: match[1].toLowerCase(), value: match[2] };
      case 'measurement':
        return { type: 'measurement', value: match[1], unit: match[2] };
      default:
        throw new Error(`Unknown type: ${type}`);
    }
  }
}
```

### Alternative 5: Configuration Files
```json
// transformations.json
{
  "ui_elements": {
    "remove_prefixes": [
      "Card:", "CardHeader:", "CardTitle:", "CardContent:", "CardFooter:"
    ],
    "shape_conversions": {
      "Circle:": "circle with",
      "Rectangle:": "rectangle with",
      "Triangle:": "triangle with"
    },
    "unit_expansions": {
      "px": " pixels",
      "×": " by "
    }
  }
}
```

```typescript
// ✅ GOOD: Configuration-driven transformer
class ConfigurableTransformer {
  constructor(private config: TransformationConfig) {}

  transform(text: string): string {
    let result = text;
    
    // Remove prefixes
    for (const prefix of this.config.ui_elements.remove_prefixes) {
      result = result.replace(new RegExp(escapeRegex(prefix), 'g'), '');
    }
    
    // Shape conversions
    for (const [from, to] of Object.entries(this.config.ui_elements.shape_conversions)) {
      result = result.replace(new RegExp(escapeRegex(from), 'g'), to);
    }
    
    // Unit expansions
    for (const [from, to] of Object.entries(this.config.ui_elements.unit_expansions)) {
      result = result.replace(new RegExp(escapeRegex(from), 'g'), to);
    }
    
    return result;
  }
}
```

## 📋 Development Rules

### Rule 1: The 3-Replace Rule
**If you need more than 2 chained `.replace()` calls, stop and use a data-driven approach.**

### Rule 2: The Contains Rule
**If you have more than 3 `.contains()` checks in a condition, use a Set or configuration.**

### Rule 3: The Equality Rule
**If you have more than 4 string equality checks for classification, use an enum or map.**

### Rule 4: The Configuration Rule
**If string patterns might change or expand, use external configuration.**

### Rule 5: The Testing Rule
**If you can't easily test individual transformation rules, refactor to use structured data.**

## 🔧 Refactoring Guidelines

### Step 1: Identify the Intent
Ask yourself:
- What is this string manipulation trying to accomplish?
- Is this classification, transformation, parsing, or validation?
- What are the business rules behind these string operations?

### Step 2: Extract the Data
Create structured representations:
- Use objects/maps for transformations
- Use sets for membership testing
- Use enums for classifications
- Use parsers for complex formats

### Step 3: Implement Incrementally
- Start with the most problematic string matching
- Gradually replace chains with structured approaches
- Add tests for individual rules
- Ensure backward compatibility during transition

### Step 4: Add Configuration
- Move hardcoded strings to configuration files
- Make rules externally configurable
- Add validation for configuration data
- Document the configuration schema

## 🚨 Exceptions: When String Matching is Acceptable

String matching is acceptable ONLY in these specific cases:

1. **Simple One-Off Replacements**
   ```typescript
   // ✅ OK: Single, simple replacement
   const cleanedId = id.replace(/[^a-zA-Z0-9]/g, '_');
   ```

2. **Framework/Library Requirements**
   ```typescript
   // ✅ OK: Required by external API
   if (event.key === "Escape") {
     handleEscape();
   }
   ```

3. **Performance-Critical Simple Cases**
   ```rust
   // ✅ OK: Hot path with proven performance benefit
   if line.starts_with('#') {
     // handle comment
   }
   ```

4. **Temporary Development Code**
   ```typescript
   // ✅ OK: Temporary debugging (must be removed before PR)
   if (DEBUG && response.includes("error")) {
     console.log("Debug error response");
   }
   ```

## 🎯 Implementation Checklist

Before implementing any string matching solution, check:

- [ ] Is this more than 2 string operations?
- [ ] Could this list of strings grow in the future?
- [ ] Do I need to test individual transformation rules?
- [ ] Would this be hard to modify for new requirements?
- [ ] Am I making assumptions about exact string formats?
- [ ] Could this be made configurable?
- [ ] Is there a more type-safe approach?

If you answered "yes" to any of these, use a structured alternative instead.

## 🔍 Code Review Guidelines

When reviewing code, look for:
- Multiple chained string operations
- Long lists of string comparisons
- Hard-coded string patterns
- String-based classification logic
- Lack of configuration flexibility

Reject PRs that use string matching when structured approaches would be better.

## 🛠️ Tools and Utilities

### String Pattern Analyzer
```typescript
// Utility to analyze string patterns for refactoring opportunities
class StringPatternAnalyzer {
  static analyzeReplacements(operations: string[]): RefactoringOpportunity {
    if (operations.length > 2) {
      return {
        severity: 'high',
        recommendation: 'Use data-driven transformation',
        patterns: this.extractPatterns(operations)
      };
    }
    return { severity: 'low', recommendation: 'Consider monitoring' };
  }
}
```

### Configuration Generator
```typescript
// Generate configuration from existing string operations
class ConfigurationGenerator {
  static fromReplacements(replacements: Array<[string, string]>): TransformationConfig {
    return {
      transformations: replacements.map(([from, to], index) => ({
        id: `rule_${index}`,
        pattern: from,
        replacement: to,
        description: `Auto-generated rule ${index}`
      }))
    };
  }
}
```

## 📚 Further Reading

- [Data-Driven Programming Principles](./data-driven-programming.md)
- [Configuration Management Best Practices](./configuration-management.md)
- [Type Safety Guidelines](./type-safety.md)
- [Performance Optimization Patterns](./performance-patterns.md)

---

**Remember: Every string matching solution should be justified. When in doubt, use structured data.**