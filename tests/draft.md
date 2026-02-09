## Main Flow
Reads input stream 2 times
1. Find suitable dialects
    - Key: Value
      - ":" is the delimiter
      - processes chunk bite by bite
        - find EoL (\r\n)
        - find delimiters
    - Single byte
      - common delimiters only
      - process chunk bite by bite
        - find EoL
        - find separators [b'\t', b',', b';', b'|', b':']
        - find terminators [b'\n', CrLf]
        - find quote chars [b'"', b'\'']
        - find escape chars [b'\\']
2. Normalize stream using dialect

## Unit Tests
- Dialects testing
    - do not test functions by themselves
    - find common csv formats (dialects)
    - call process_chunk
      - assert dialect as expected
      - asser normalize result