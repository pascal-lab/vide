export const TOML_LANGUAGE_ID = 'toml';

export const TOML_LANGUAGE_CONFIGURATION = {
  comments: {
    lineComment: '#',
  },
  brackets: [['[', ']']],
  autoClosingPairs: [
    { open: '[', close: ']' },
    { open: '"', close: '"' },
    { open: "'", close: "'" },
  ],
};
