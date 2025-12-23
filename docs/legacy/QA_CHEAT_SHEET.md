# QA Cheat Sheet: UI-Guided Visual Token Selection

**Quick Reference Guide for Quality Assurance Testing**

## 🚀 **Quick QA Commands**

### **Full Validation Suite**

```bash
./scripts/qa-full-validation.sh
# Runs complete QA validation (9 phases, ~2-3 minutes)
# Expected: All tests pass, production ready
```

### **Performance Benchmarking**

```bash
./scripts/benchmark-token-selection.sh
# Tests processing speed and reduction rates
# Expected: 4K <100ms, HD <80ms, 65%+ reduction
```

### **Multi-Monitor Testing**

```bash
./scripts/test-multi-monitor-scenarios.sh
# Tests all display configurations
# Expected: Single/dual/triple/quad monitor support
```

### **Basic Functionality**

```bash
./scripts/test-ui-token-selection.sh
# Basic feature validation
# Expected: 18/18 tests passed
```

## ✅ **QA Checklist**

### **Pre-Testing**

- [ ] `cargo check --manifest-path src-tauri/Cargo.toml --quiet` passes
- [ ] All dependencies installed (lru, image, tokio, serde)
- [ ] Development environment running (`RUST_LOG=debug bun run tauri dev`)

### **Core Functionality**

- [ ] Token selection API parameter works (`enable_token_selection: true`)
- [ ] Graceful fallback when token selection fails
- [ ] Feature can be disabled (`enable_token_selection: false`)
- [ ] RGB analysis producing correct results
- [ ] Token reduction meeting targets (65-75% for 4K, 70-80% for HD)

### **Performance**

- [ ] Processing time <100ms for 4K screenshots
- [ ] Processing time <80ms for HD screenshots
- [ ] Memory usage stable (no leaks)
- [ ] Concurrent processing working
- [ ] 33%+ computational cost reduction achieved

### **Multi-Monitor**

- [ ] Single monitor configurations working
- [ ] Dual monitor setups (horizontal/vertical) validated
- [ ] Triple+ monitor configurations tested
- [ ] Display-aware optimization functional
- [ ] Cross-display redundancy elimination working

### **Error Handling**

- [ ] Invalid input handled gracefully
- [ ] Resource exhaustion managed
- [ ] API integration errors recovered
- [ ] Timeout mechanisms working

### **Integration**

- [ ] Computer Use workflow complete
- [ ] Multi-action sequences functional
- [ ] UI element detection preserved
- [ ] Screenshots still capture correctly

## 🎯 **Success Criteria**

| Category | Target | Validation |
|----------|--------|------------|
| **Tests** | 18/18 pass | `./scripts/test-ui-token-selection.sh` |
| **4K Performance** | <100ms, 65%+ reduction | `./scripts/benchmark-token-selection.sh` |
| **HD Performance** | <80ms, 70%+ reduction | `./scripts/benchmark-token-selection.sh` |
| **Multi-Monitor** | All configs working | `./scripts/test-multi-monitor-scenarios.sh` |
| **Error Handling** | Graceful fallbacks | Manual testing + QA scripts |
| **Integration** | Computer Use working | Frontend testing |

## 🚨 **Common Issues & Fixes**

### **Compilation Errors**

```bash
# Check for errors
cargo check --manifest-path src-tauri/Cargo.toml

# Common fixes:
# - Missing dependencies in Cargo.toml
# - Type mismatches (TokenSelectionError vs TokenSelectorError)
# - Missing imports or exports
```

### **Function Not Found Errors**

```bash
# Check exports in lib.rs
grep "ui_token_selection::" src-tauri/src/lib.rs

# Check module declaration
grep "ui_token_selection" src-tauri/src/commands/mod.rs
```

### **Performance Issues**

```bash
# Run benchmarks to identify bottlenecks
./scripts/benchmark-token-selection.sh

# Check for memory leaks
# Monitor processing times
# Validate reduction rates
```

### **Multi-Monitor Problems**

```bash
# Test specific configurations
VERBOSE=true ./scripts/test-multi-monitor-scenarios.sh

# Check display detection
# Validate resolution handling
# Test cross-display features
```

## 📋 **Manual Testing Steps**

### **Frontend Testing**

1. Open Juno application
2. Navigate to Computer Use tools
3. Enable token selection in settings (if available)
4. Take screenshots with `enable_token_selection: true`
5. Verify metadata includes token reduction info
6. Test with `enable_token_selection: false`
7. Validate graceful fallback on errors

### **API Testing**

```javascript
// Test token selection enabled
const result = await invoke('anthropic_computer_use_action', {
  action: {
    action: 'screenshot',
    enable_token_selection: true
  }
});

// Verify response structure
assert(result.token_selection !== undefined);
assert(result.token_selection.reduction_percentage > 0);
```

### **Multi-Monitor Testing**

1. Set up different monitor configurations
2. Test screenshot capture on each display
3. Verify token reduction varies by display
4. Test cross-display operations
5. Validate layout detection

## 📊 **Expected Results**

### **Successful QA Run**

```
✅ All functional tests passed
✅ Performance targets met  
✅ Multi-monitor scenarios validated
✅ Error handling verified
✅ Integration tests successful
🎉 QA VALIDATION COMPLETE - READY FOR PRODUCTION
```

### **Performance Benchmarks**

```
4K Display: 85ms, 68% reduction ✅
HD Display: 62ms, 74% reduction ✅  
Multi-Monitor: 145ms, 76% reduction ✅
Memory Usage: +2.3MB ✅
Stress Test: 45ms average ✅
```

### **Multi-Monitor Results**

```
Single_4K: 89ms, 67% ✅
Dual_HD_Horizontal: 142ms, 77% ✅
Triple_Linear: 195ms, 79% ✅
Display Priority: Primary 75.5% > Secondary 68.2% ✅
Cross-Display Redundancy: 91.3% efficiency ✅
```

## 🔧 **Troubleshooting**

### **QA Script Failures**

1. Check script permissions: `chmod +x scripts/*.sh`
2. Verify working directory: `cd /path/to/juno`
3. Check dependencies: `which bc cargo grep`
4. Review error output for specific failures

### **Performance Issues**

1. Check system resources (CPU, memory)
2. Verify no other intensive processes running
3. Test with smaller iterations: `ITERATIONS=3 ./scripts/benchmark-token-selection.sh`
4. Review algorithm complexity and optimization

### **Test Environment Issues**

1. Ensure clean git state (for some tests)
2. Verify Rust toolchain version
3. Check for conflicting processes
4. Validate test data and configurations

---

**Quick Start**: Run `./scripts/qa-full-validation.sh` for complete validation
**Documentation**: See `docs/QA_GUIDE_UI_TOKEN_SELECTION.md` for detailed procedures
**Support**: Review error logs and test output for specific guidance
