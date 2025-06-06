/// <reference types="vitest" />
import { filterTranscriptionForDisplay } from './transcriptionFilter';

describe('filterTranscriptionForDisplay', () => {
  it('should remove basic capital text in brackets', () => {
    expect(filterTranscriptionForDisplay('Hello [BLANK AUDIO] world')).toBe('Hello world');
    expect(filterTranscriptionForDisplay('Test [SILENCE] case')).toBe('Test case');
    expect(filterTranscriptionForDisplay('Music [NOISE] playing')).toBe('Music playing');
  });

  it('should remove multiple capital sequences', () => {
    expect(filterTranscriptionForDisplay('Start [BLANK AUDIO] middle [SILENCE] end')).toBe('Start middle end');
  });

  it('should handle spaces inside brackets', () => {
    expect(filterTranscriptionForDisplay('Test [ BLANK AUDIO ] spaces')).toBe('Test spaces');
    expect(filterTranscriptionForDisplay('Test [ MUSIC PLAYING ] spaces')).toBe('Test spaces');
  });

  it('should remove text with only capital sequences', () => {
    expect(filterTranscriptionForDisplay('[BLANK AUDIO]')).toBe('');
    expect(filterTranscriptionForDisplay('[SILENCE] [NOISE]')).toBe('');
  });

  it('should clean up extra whitespace', () => {
    expect(filterTranscriptionForDisplay('  Multiple   spaces  ')).toBe('Multiple spaces');
  });

  it('should handle mixed scenarios', () => {
    expect(filterTranscriptionForDisplay('  Start [BACKGROUND NOISE]   middle   [SILENCE]  end  '))
      .toBe('Start middle end');
  });

  it('should preserve normal text without capital sequences', () => {
    expect(filterTranscriptionForDisplay('Normal transcription text')).toBe('Normal transcription text');
  });

  it('should preserve lowercase and mixed-case text in brackets', () => {
    expect(filterTranscriptionForDisplay('Keep [this text] and [This Too]')).toBe('Keep [this text] and [This Too]');
  });

  it('should remove single capital letters in brackets', () => {
    expect(filterTranscriptionForDisplay('Remove [A] single letters')).toBe('Remove single letters');
  });

  it('should handle empty strings', () => {
    expect(filterTranscriptionForDisplay('')).toBe('');
  });

  it('should handle complex patterns', () => {
    expect(filterTranscriptionForDisplay('Hello [COUGHING] I am [speaking] with [BACKGROUND MUSIC] noise'))
      .toBe('Hello I am [speaking] with noise');
  });
});