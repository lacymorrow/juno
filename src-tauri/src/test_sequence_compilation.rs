// Test file to verify sequence.rs compiles correctly
#[cfg(test)]
mod test_sequence_compilation {
    use crate::events::sequence::*;
    use tauri::test::*;
    
    #[test]
    fn test_sequence_traits_compile() {
        // Test that SequencedEvent works with Clone constraint
        let event = SequencedEvent::new("test payload".to_string());
        let _cloned = event.clone();
        
        // Test that sequence numbers work
        let seq1 = next_sequence();
        let seq2 = next_sequence();
        assert!(seq2 > seq1);
    }
    
    #[test]
    fn test_sequenced_emitter_trait() {
        // This just needs to compile to verify the trait is properly defined
        fn uses_sequenced_emitter<T: SequencedEmitter>(_emitter: &T) {
            // Function body doesn't matter, we just need the trait bound to compile
        }
    }
}