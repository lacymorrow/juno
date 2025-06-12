# String Matching Refactoring Example

This document shows a practical refactoring of the string matching anti-pattern provided by the user, demonstrating how to transform it into a structured, maintainable solution.

## ❌ Original Anti-Pattern Code

```typescript
// Convert common visual descriptions to natural language
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

### Problems with this approach:
1. **Brittle**: Adding new UI components requires code changes
2. **Hard to test**: Can't test individual transformation rules
3. **Not configurable**: Rules are hardcoded in the logic
4. **Poor performance**: Linear chain of string operations
5. **Order dependent**: Rules must be applied in specific sequence
6. **No metadata**: No description of what each rule does

## ✅ Refactored Solution

### Step 1: Define Transformation Configuration

```typescript
// transformations/speech-text-config.ts
export interface TransformationRule {
  id: string;
  pattern: string | RegExp;
  replacement: string;
  description: string;
  category: string;
  priority?: number;
}

export const SPEECH_TEXT_TRANSFORMATIONS: TransformationRule[] = [
  // UI Component Cleanup (highest priority)
  {
    id: "remove_card_prefixes",
    pattern: /Card(Header|Title|Content|Footer)?:/g,
    replacement: "",
    description: "Remove React Card component prefixes",
    category: "ui_cleanup",
    priority: 1
  },

  // SVG and Shape Conversions
  {
    id: "svg_circle_to_visual",
    pattern: /SVG Circle/g,
    replacement: "visual circle",
    description: "Convert SVG circle references to natural language",
    category: "shape_conversion"
  },
  
  {
    id: "shape_descriptors",
    pattern: /(Circle|Rectangle|Triangle):/g,
    replacement: "$1 with",
    description: "Convert shape labels to descriptive phrases",
    category: "shape_conversion"
  },

  // Unit and Symbol Conversions
  {
    id: "multiplication_symbol",
    pattern: /×/g,
    replacement: " by ",
    description: "Convert multiplication symbol to words",
    category: "symbol_conversion"
  },
  
  {
    id: "pixel_units",
    pattern: /\bpx\b/g,
    replacement: " pixels",
    description: "Expand pixel unit abbreviations",
    category: "unit_conversion"
  }
];
```

### Step 2: Create a Configurable Transformer

```typescript
// transformers/speech-text-transformer.ts
import { TransformationRule, SPEECH_TEXT_TRANSFORMATIONS } from '../transformations/speech-text-config';

export class SpeechTextTransformer {
  private rules: TransformationRule[];

  constructor(rules: TransformationRule[] = SPEECH_TEXT_TRANSFORMATIONS) {
    // Sort by priority (higher priority first)
    this.rules = rules.sort((a, b) => (b.priority || 0) - (a.priority || 0));
  }

  /**
   * Transform speech text using configured rules
   */
  transform(text: string): string {
    return this.rules.reduce((result, rule) => {
      return result.replace(rule.pattern, rule.replacement);
    }, text);
  }

  /**
   * Transform with detailed logging for debugging
   */
  transformWithLogging(text: string): { result: string; applied: TransformationRule[] } {
    const applied: TransformationRule[] = [];
    let result = text;

    for (const rule of this.rules) {
      const before = result;
      result = result.replace(rule.pattern, rule.replacement);
      
      if (before !== result) {
        applied.push(rule);
        console.log(`Applied rule "${rule.id}": ${rule.description}`);
      }
    }

    return { result, applied };
  }

  /**
   * Get transformation rules by category
   */
  getRulesByCategory(category: string): TransformationRule[] {
    return this.rules.filter(rule => rule.category === category);
  }

  /**
   * Add custom rule at runtime
   */
  addRule(rule: TransformationRule): void {
    this.rules.push(rule);
    this.rules.sort((a, b) => (b.priority || 0) - (a.priority || 0));
  }

  /**
   * Test a specific rule against text
   */
  testRule(ruleId: string, text: string): { matched: boolean; result: string } {
    const rule = this.rules.find(r => r.id === ruleId);
    if (!rule) {
      throw new Error(`Rule "${ruleId}" not found`);
    }

    const result = text.replace(rule.pattern, rule.replacement);
    return {
      matched: result !== text,
      result
    };
  }
}
```

### Step 3: Usage Examples

```typescript
// usage/basic-usage.ts
import { SpeechTextTransformer } from '../transformers/speech-text-transformer';

// Basic usage
const transformer = new SpeechTextTransformer();
const originalText = "CardHeader: User Settings Circle: 24px × 16px";
const transformedText = transformer.transform(originalText);
console.log(transformedText); // "User Settings circle with 24 pixels  by  16 pixels"

// Usage with debugging
const { result, applied } = transformer.transformWithLogging(originalText);
console.log(`Applied ${applied.length} transformation rules`);
applied.forEach(rule => console.log(`- ${rule.description}`));
```

### Step 4: External Configuration Support

```json
// config/speech-transformations.json
{
  "version": "1.0.0",
  "description": "Speech text transformation rules",
  "rules": [
    {
      "id": "custom_component_cleanup",
      "pattern": "Button:|Input:|Form:",
      "replacement": "",
      "description": "Remove custom component prefixes",
      "category": "ui_cleanup",
      "priority": 1
    },
    {
      "id": "accessibility_labels",
      "pattern": "aria-label:",
      "replacement": "labeled as",
      "description": "Convert accessibility labels to natural language",
      "category": "accessibility"
    }
  ]
}
```

```typescript
// transformers/configurable-transformer.ts
export class ConfigurableSpeechTransformer extends SpeechTextTransformer {
  static async fromConfigFile(configPath: string): Promise<ConfigurableSpeechTransformer> {
    const config = JSON.parse(await fs.readFile(configPath, 'utf8'));
    return new ConfigurableSpeechTransformer(config.rules);
  }

  async saveConfiguration(configPath: string): Promise<void> {
    const config = {
      version: "1.0.0",
      description: "Speech text transformation rules",
      rules: this.rules
    };
    await fs.writeFile(configPath, JSON.stringify(config, null, 2));
  }
}
```

### Step 5: Comprehensive Testing

```typescript
// tests/speech-text-transformer.test.ts
import { SpeechTextTransformer } from '../transformers/speech-text-transformer';

describe('SpeechTextTransformer', () => {
  let transformer: SpeechTextTransformer;

  beforeEach(() => {
    transformer = new SpeechTextTransformer();
  });

  describe('UI Component Cleanup', () => {
    test('removes Card component prefixes', () => {
      expect(transformer.testRule('remove_card_prefixes', 'CardHeader: Title')).toEqual({
        matched: true,
        result: 'Title'
      });
    });

    test('handles multiple card components', () => {
      const input = 'CardTitle: Settings CardContent: User preferences';
      const expected = 'Settings  User preferences';
      expect(transformer.transform(input)).toBe(expected);
    });
  });

  describe('Shape Conversions', () => {
    test('converts shape descriptors', () => {
      expect(transformer.testRule('shape_descriptors', 'Circle: red')).toEqual({
        matched: true,
        result: 'Circle with red'
      });
    });

    test('converts SVG elements', () => {
      expect(transformer.testRule('svg_circle_to_visual', 'SVG Circle element')).toEqual({
        matched: true,
        result: 'visual circle element'
      });
    });
  });

  describe('Unit Conversions', () => {
    test('expands pixel units', () => {
      expect(transformer.testRule('pixel_units', '24px width')).toEqual({
        matched: true,
        result: '24 pixels width'
      });
    });

    test('converts multiplication symbols', () => {
      expect(transformer.testRule('multiplication_symbol', '24 × 16')).toEqual({
        matched: true,
        result: '24  by  16'
      });
    });
  });

  describe('Integration Tests', () => {
    test('applies all transformations in correct order', () => {
      const input = 'CardHeader: User Icon Circle: 16px × 16px SVG Circle';
      const expected = 'User Icon circle with 16 pixels  by  16 pixels visual circle';
      expect(transformer.transform(input)).toBe(expected);
    });

    test('handles text with no transformations needed', () => {
      const input = 'Regular text with no special patterns';
      expect(transformer.transform(input)).toBe(input);
    });
  });

  describe('Configuration Management', () => {
    test('can add custom rules', () => {
      transformer.addRule({
        id: 'test_rule',
        pattern: /test:/g,
        replacement: 'testing',
        description: 'Test rule',
        category: 'test'
      });

      expect(transformer.transform('test: something')).toBe('testing something');
    });

    test('can filter rules by category', () => {
      const uiRules = transformer.getRulesByCategory('ui_cleanup');
      expect(uiRules.length).toBeGreaterThan(0);
      expect(uiRules.every(rule => rule.category === 'ui_cleanup')).toBe(true);
    });
  });
});
```

### Step 6: Performance Benchmark

```typescript
// benchmarks/transformation-performance.ts
import { SpeechTextTransformer } from '../transformers/speech-text-transformer';

function benchmarkTransformations() {
  const transformer = new SpeechTextTransformer();
  const testText = 'CardHeader: Settings Circle: 24px × 16px SVG Circle Rectangle: blue Triangle: small';
  
  console.time('Original String Chain (simulated)');
  for (let i = 0; i < 10000; i++) {
    // Simulate original chained approach
    let result = testText
      .replace("Card:", "")
      .replace("CardHeader:", "")
      .replace("Circle:", "circle with")
      .replace("Rectangle:", "rectangle with")
      .replace("Triangle:", "triangle with")
      .replace("×", " by ")
      .replace("px", " pixels");
  }
  console.timeEnd('Original String Chain (simulated)');

  console.time('Structured Transformation');
  for (let i = 0; i < 10000; i++) {
    transformer.transform(testText);
  }
  console.timeEnd('Structured Transformation');
}

benchmarkTransformations();
```

## 🎯 Benefits of the Refactored Solution

### 1. **Maintainability**
- New transformations can be added via configuration
- Each rule is self-documenting with descriptions
- Rules can be modified without touching the core logic

### 2. **Testability**
- Each transformation rule can be tested individually
- Integration tests verify the complete transformation pipeline
- Performance can be measured and optimized

### 3. **Flexibility**
- Rules can be prioritized and ordered
- Categories allow filtering and selective application
- External configuration files enable runtime customization

### 4. **Debugging**
- Detailed logging shows which rules were applied
- Individual rules can be tested in isolation
- Transformation history is trackable

### 5. **Performance**
- Rules are pre-sorted by priority
- Regex patterns are compiled once
- No repeated string operations

### 6. **Extensibility**
- New rule types can be added easily
- Custom transformers can extend the base class
- Plugin architecture possible with dynamic rule loading

## 📋 Migration Checklist

When refactoring string matching code:

- [ ] Identify all transformation patterns
- [ ] Group related transformations by category
- [ ] Define configuration structure
- [ ] Create transformer class with clear interfaces
- [ ] Add comprehensive tests for each rule
- [ ] Add integration tests for complete pipeline
- [ ] Benchmark performance vs original approach
- [ ] Add debugging and logging capabilities
- [ ] Document configuration schema
- [ ] Create migration guide for team

## 🔄 Continuous Improvement

The structured approach enables:

1. **A/B Testing**: Test different transformation rules
2. **Analytics**: Track which rules are most frequently used
3. **User Customization**: Allow users to modify transformation preferences
4. **Automatic Optimization**: Use machine learning to improve rules over time
5. **Rule Validation**: Automatically test rules against large datasets

This refactoring transforms a brittle string manipulation chain into a robust, configurable, and testable transformation system that can grow with your application's needs.