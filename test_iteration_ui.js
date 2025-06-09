// Test script to verify get_agent_execution_progress command
import { invoke } from '@tauri-apps/api/core';

async function testIterationUI() {
    try {
        console.log('Testing get_agent_execution_progress command...');

        const result = await invoke('get_agent_execution_progress');

        console.log('✅ Success! Command returned:', result);
        console.log('  - is_executing:', result.is_executing);
        console.log('  - current_step:', result.current_step);
        console.log('  - max_steps:', result.max_steps);
        console.log('  - remaining_steps:', result.remaining_steps);
        console.log('  - progress_percentage:', result.progress_percentage);

        return true;
    } catch (error) {
        console.error('❌ Error testing command:', error);
        return false;
    }
}

// Run the test
testIterationUI().then(success => {
    if (success) {
        console.log('🎉 Iteration UI test passed!');
    } else {
        console.log('💥 Iteration UI test failed!');
    }
});
