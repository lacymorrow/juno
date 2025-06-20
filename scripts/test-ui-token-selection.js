//! UI-Guided Visual Token Selection Validation Script
//!
//! Comprehensive testing suite for Week 3 implementation validation
//! Tests performance benchmarking, multi-monitor support, and cost reduction targets

const { invoke } = require('@tauri-apps/api/tauri');

// Test Configuration
const TEST_CONFIG = {
    // Performance targets from ShowUI research
    EXPECTED_REDUCTION_TARGETS: {
        '4K': 0.65,      // 65% minimum token reduction for 4K displays
        'HD': 0.70,      // 70% minimum token reduction for HD displays
        'MULTI': 0.70    // 70% minimum for multi-monitor setups
    },

    // Processing time targets (milliseconds)
    PROCESSING_TIME_TARGETS: {
        'STANDARD': 100,  // <100ms for standard screenshots
        'COMPLEX': 250,   // <250ms for complex multi-monitor scenarios
        'BENCHMARK': 500  // <500ms for full benchmark suite
    }
};

class UITokenSelectionValidator {
    constructor() {
        this.results = {
            tests_run: 0,
            tests_passed: 0,
            tests_failed: 0,
            performance_metrics: {},
            errors: []
        };
    }

    async runComprehensiveValidation() {
        console.log('🚀 Starting UI-Guided Visual Token Selection Validation...\n');

        try {
            // Step 1: Initialize system
            await this.testSystemInitialization();

            // Step 2: Performance benchmarking
            await this.testPerformanceBenchmarking();

            // Step 3: Multi-monitor optimization
            await this.testMultiMonitorOptimization();

            // Step 4: Cost reduction validation
            await this.testCostReductionTargets();

            // Step 5: Integration validation
            await this.testComputerUseIntegration();

            // Generate final report
            this.generateValidationReport();

            return this.results;

        } catch (error) {
            console.error('❌ Critical validation error:', error);
            this.results.errors.push(error.toString());
            return this.results;
        }
    }

    async testSystemInitialization() {
        console.log('📋 Testing System Initialization...');
        this.results.tests_run++;

        try {
            const startTime = Date.now();
            const result = await invoke('initialize_ui_token_selection');
            const processingTime = Date.now() - startTime;

            if (result.success) {
                console.log('✅ System initialization successful');
                console.log(`   Processing time: ${processingTime}ms`);
                this.results.tests_passed++;
                this.results.performance_metrics.initialization_time = processingTime;
            } else {
                throw new Error(result.error || 'Initialization failed');
            }
        } catch (error) {
            console.log('❌ System initialization failed:', error.message);
            this.results.tests_failed++;
            this.results.errors.push(`Initialization: ${error.message}`);
        }

        console.log('');
    }

    async testPerformanceBenchmarking() {
        console.log('📊 Testing Performance Benchmarking...');
        this.results.tests_run++;

        try {
            const startTime = Date.now();
            const benchmarkResult = await invoke('run_performance_benchmark');
            const totalTime = Date.now() - startTime;

            if (benchmarkResult.success && benchmarkResult.results.length > 0) {
                console.log('✅ Performance benchmarking successful');
                console.log(`   Total benchmark time: ${totalTime}ms`);
                console.log(`   Scenarios tested: ${benchmarkResult.results.length}`);

                // Validate against targets
                const avgReduction = benchmarkResult.results.reduce((sum, r) =>
                    sum + r.reduction_percentage, 0) / benchmarkResult.results.length;

                console.log(`   Average token reduction: ${avgReduction.toFixed(1)}%`);

                if (totalTime < TEST_CONFIG.PROCESSING_TIME_TARGETS.BENCHMARK) {
                    console.log(`   ✅ Benchmark time target met (${totalTime}ms < ${TEST_CONFIG.PROCESSING_TIME_TARGETS.BENCHMARK}ms)`);
                } else {
                    console.log(`   ⚠️ Benchmark time exceeded target (${totalTime}ms > ${TEST_CONFIG.PROCESSING_TIME_TARGETS.BENCHMARK}ms)`);
                }

                this.results.tests_passed++;
                this.results.performance_metrics.benchmark_time = totalTime;
                this.results.performance_metrics.average_reduction = avgReduction;
                this.results.performance_metrics.benchmark_results = benchmarkResult.results;

            } else {
                throw new Error('Benchmark failed or returned no results');
            }
        } catch (error) {
            console.log('❌ Performance benchmarking failed:', error.message);
            this.results.tests_failed++;
            this.results.errors.push(`Benchmarking: ${error.message}`);
        }

        console.log('');
    }

    async testMultiMonitorOptimization() {
        console.log('🖥️ Testing Multi-Monitor Optimization...');
        this.results.tests_run++;

        try {
            const startTime = Date.now();
            const multiMonitorResult = await invoke('test_multi_monitor_optimization');
            const processingTime = Date.now() - startTime;

            if (multiMonitorResult.success) {
                console.log('✅ Multi-monitor optimization successful');
                console.log(`   Processing time: ${processingTime}ms`);
                console.log(`   Displays optimized: ${multiMonitorResult.displays_tested}`);

                if (processingTime < TEST_CONFIG.PROCESSING_TIME_TARGETS.COMPLEX) {
                    console.log(`   ✅ Processing time target met`);
                } else {
                    console.log(`   ⚠️ Processing time exceeded target`);
                }

                this.results.tests_passed++;
                this.results.performance_metrics.multi_monitor_time = processingTime;
                this.results.performance_metrics.displays_tested = multiMonitorResult.displays_tested;

            } else {
                throw new Error(multiMonitorResult.error || 'Multi-monitor optimization failed');
            }
        } catch (error) {
            console.log('❌ Multi-monitor optimization failed:', error.message);
            this.results.tests_failed++;
            this.results.errors.push(`Multi-monitor: ${error.message}`);
        }

        console.log('');
    }

    async testCostReductionTargets() {
        console.log('💰 Testing Cost Reduction Targets...');
        this.results.tests_run++;

        try {
            const validationResult = await invoke('validate_cost_reduction_target');

            if (validationResult.success && validationResult.meets_target) {
                console.log('✅ Cost reduction target validation successful');
                console.log(`   Target met: ${validationResult.meets_target}`);
                console.log(`   Current reduction: ${validationResult.current_reduction.toFixed(1)}%`);
                console.log(`   Target reduction: ${validationResult.target_reduction.toFixed(1)}%`);

                this.results.tests_passed++;
                this.results.performance_metrics.cost_reduction_validated = true;
                this.results.performance_metrics.current_reduction = validationResult.current_reduction;

            } else {
                throw new Error(`Cost reduction target not met: ${validationResult.current_reduction}% < ${validationResult.target_reduction}%`);
            }
        } catch (error) {
            console.log('❌ Cost reduction validation failed:', error.message);
            this.results.tests_failed++;
            this.results.errors.push(`Cost reduction: ${error.message}`);
        }

        console.log('');
    }

    async testComputerUseIntegration() {
        console.log('🤖 Testing Computer Use Integration...');
        this.results.tests_run++;

        try {
            // Test a simple screenshot with token selection enabled
            const testResult = await invoke('test_ui_token_selection', {
                config: {
                    enable_token_selection: true,
                    multi_monitor_optimization: true,
                    performance_tracking: true,
                    target_reduction_percentage: 33.0
                }
            });

            if (testResult.success) {
                console.log('✅ Computer Use integration successful');
                console.log(`   Original tokens: ${testResult.original_tokens}`);
                console.log(`   Reduced tokens: ${testResult.reduced_tokens}`);
                console.log(`   Reduction: ${testResult.reduction_percentage.toFixed(1)}%`);
                console.log(`   Processing time: ${testResult.processing_time_ms}ms`);

                this.results.tests_passed++;
                this.results.performance_metrics.integration_test = testResult;

            } else {
                throw new Error(testResult.error || 'Integration test failed');
            }
        } catch (error) {
            console.log('❌ Computer Use integration failed:', error.message);
            this.results.tests_failed++;
            this.results.errors.push(`Integration: ${error.message}`);
        }

        console.log('');
    }

    generateValidationReport() {
        console.log('📋 VALIDATION REPORT SUMMARY');
        console.log('=' .repeat(50));
        console.log(`Tests Run: ${this.results.tests_run}`);
        console.log(`Tests Passed: ${this.results.tests_passed}`);
        console.log(`Tests Failed: ${this.results.tests_failed}`);
        console.log(`Success Rate: ${((this.results.tests_passed / this.results.tests_run) * 100).toFixed(1)}%`);
        console.log('');

        if (this.results.performance_metrics.average_reduction) {
            console.log('📊 PERFORMANCE METRICS');
            console.log('-'.repeat(30));
            console.log(`Average Token Reduction: ${this.results.performance_metrics.average_reduction.toFixed(1)}%`);
            console.log(`Benchmark Time: ${this.results.performance_metrics.benchmark_time}ms`);
            if (this.results.performance_metrics.cost_reduction_validated) {
                console.log(`Cost Reduction Target: ✅ MET (${this.results.performance_metrics.current_reduction.toFixed(1)}%)`);
            }
            console.log('');
        }

        if (this.results.errors.length > 0) {
            console.log('❌ ERRORS ENCOUNTERED');
            console.log('-'.repeat(30));
            this.results.errors.forEach((error, index) => {
                console.log(`${index + 1}. ${error}`);
            });
            console.log('');
        }

        // Overall status
        if (this.results.tests_failed === 0) {
            console.log('🎉 ALL VALIDATION TESTS PASSED!');
            console.log('✅ UI-Guided Visual Token Selection is ready for production');
        } else {
            console.log('⚠️ Some validation tests failed');
            console.log('🔧 Review errors and rerun validation');
        }

        console.log('=' .repeat(50));
    }
}

// Export for use as module or run directly
if (typeof module !== 'undefined' && module.exports) {
    module.exports = { UITokenSelectionValidator, TEST_CONFIG };
} else {
    // Run validation if called directly
    const validator = new UITokenSelectionValidator();
    validator.runComprehensiveValidation()
        .then(results => {
            console.log('\n🏁 Validation completed');
            process.exit(results.tests_failed === 0 ? 0 : 1);
        })
        .catch(error => {
            console.error('💥 Critical validation failure:', error);
            process.exit(1);
        });
}
