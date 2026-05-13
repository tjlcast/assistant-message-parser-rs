# Assistant Message Parser RS

Rust implementation of `src/examples/ai_chat_modular/experiments/assistant_message_parser.py`.

The crate is intentionally small and wasm-friendly:

- no filesystem, process, networking, thread, or timer APIs;
- no native runtime dependencies; wasm builds use `js-sys` and `wasm-bindgen`;
- parser state is incremental, so callers can feed streaming chunks without reparsing the full message externally;
- library output uses owned Rust structs, with a `wasm-bindgen` wrapper for Node.js consumers.

## Behavior

The parser emits two content block variants:

- `ContentBlock::Text(TextContent)` for normal assistant text;
- `ContentBlock::ToolUse(ToolUse)` for XML-like tool calls such as `<read_file><path>src/lib.rs</path></read_file>`.

It follows the Python implementation closely:

- text blocks are created while partial text streams in, with leading/trailing whitespace trimmed from each text block;
- recognized tool opening tags close the current text block and append a partial tool block immediately;
- parameter values are updated during streaming;
- non-`content` parameters are trimmed when their closing tag is seen;
- `content` parameters preserve internal newlines and strip only one leading and one trailing newline;
- `write_to_file` refreshes `content` from the last `</content>` so embedded `</content>` strings inside file content are preserved;
- malformed or unknown XML-like tags stay in text instead of becoming tool blocks;
- `finalize_content_blocks()` marks all blocks as complete and trims text blocks;
- messages larger than 1 MiB return `ParserError::MessageTooLarge`;
- parameter values larger than 100 KiB are abandoned gracefully, matching the Python parser's safe-state behavior.

`AssistantMessageParser::default()` recognizes the default tool schema from `src/lib.rs`:

| Tool | Parameters |
| --- | --- |
| `write_to_file` | `path`, `content`, `line_count` |
| `update_todo_list` | `todos` |
| `search_files` | `path`, `regex`, `file_pattern` |
| `search_and_replace` | `path`, `search`, `replace`, `start_line`, `end_line`, `use_regex`, `ignore_case` |
| `read_file` | `args` |
| `list_files` | `path`, `recursive` |
| `insert_content` | `path`, `line`, `content` |
| `execute_command` | `command`, `cwd` |
| `attempt_completion` | `result` |
| `ask_followup_question` | `question`, `follow_up` |
| `new_task` | `mode`, `message` |
| `workflow_search` | `q`, `trigger`, `complexity`, `active_only`, `page`, `per_page` |

## Usage

```rust
use assistant_message_parser::{AssistantMessageParser, ContentBlock};

let mut parser = AssistantMessageParser::default();
let blocks = parser
    .process_chunk("<read_file><args>src/main.rs</args></read_file>")
    .unwrap();

match &blocks[0] {
    ContentBlock::ToolUse(tool) => {
        assert_eq!(tool.name, "read_file");
        assert_eq!(tool.params.get("args").unwrap(), "src/main.rs");
    }
    ContentBlock::Text(_) => unreachable!(),
}
```

For custom tools:

```rust
use assistant_message_parser::AssistantMessageParser;

let mut parser = AssistantMessageParser::new(
    Some(vec!["read_file".into(), "write_to_file".into()]),
    Some(vec!["path".into(), "content".into()]),
);

let blocks = parser
    .process_chunk("<read_file><path>src/main.rs</path></read_file>")
    .unwrap();
```

## Node.js Wasm Usage

Build the Node.js wasm package into `pkg/`:

```bash
npm run build:wasm
```

Then require the generated package:

```js
const { AssistantMessageParser } = require("./pkg/assistant_message_parser.js");

const parser = new AssistantMessageParser(["read_file"], ["path"]);
const blocks = parser.processChunk("<read_file><path>src/main.rs</path></read_file>");

console.log(blocks[0]);
// {
//   type: "tool_use",
//   name: "read_file",
//   params: { path: "src/main.rs" },
//   partial: false,
//   xml: "<read_file>\n<path>src/main.rs</path>\n</read_file>"
// }
```

Pass `undefined`/`null` for either constructor argument to use the Rust defaults for that list. The wasm wrapper also exposes `AssistantMessageParser.default()`, `reset()`, `getContentBlocks()`, `finalizeContentBlocks()`, and `nextTextChunk()`. `processChunk()` throws a JavaScript `Error` when the Rust parser returns `ParserError::MessageTooLarge`.

## Classic Streaming Case

This mirrors the common LLM streaming flow: show normal assistant text as soon as it is safe to display, while separately collecting only completed tool calls.

```rust
use assistant_message_parser::{AssistantMessageParser, ContentBlock};

let mut parser = AssistantMessageParser::new(
    Some(vec!["read_file".into(), "search_files".into()]),
    Some(vec!["path".into(), "regex".into()]),
);

let response_chunks = [
    "I will inspect the file. ",
    "<read_file><path>src/main.rs</path></read_file>",
    " Then I will search. ",
    "<search_files><regex>fn main</regex><path>src</path></search_files>",
    " Done.",
];

let mut show_text = String::new();
let mut completed_tool_xml = Vec::new();
let mut seen_completed_tool_count = 0;

for chunk in response_chunks {
    let parsed_blocks = parser.process_chunk(chunk).unwrap();

    while let Some(text_chunk) = parser.next_text_chunk() {
        if !text_chunk.trim().is_empty() {
            // Send this delta to the UI.
            show_text.push_str(&text_chunk);
        }
    }

    let completed_tools = parsed_blocks
        .iter()
        .filter_map(|block| match block {
            ContentBlock::ToolUse(tool) if !tool.partial => Some(tool),
            _ => None,
        })
        .collect::<Vec<_>>();

    for tool in completed_tools.iter().skip(seen_completed_tool_count) {
        completed_tool_xml.push(tool.to_xml());
    }
    seen_completed_tool_count = completed_tools.len();
}

assert_eq!(show_text, "I will inspect the file. Then I will search. Done.");
assert_eq!(completed_tool_xml.len(), 2);
```

`next_text_chunk()` intentionally holds back text that may still be the prefix of a tool tag, so UI display does not briefly show `<read_file` before the parser can decide whether it is plain text or a real tool call. It emits the accumulated text-only stream and joins separate text blocks with a single space.

## Diagrams

- `docs/processing-flow.svg` shows the per-character dispatch order inside `process_chunk()`.
- `docs/state-machine.svg` shows the parser's implicit text, tool, and parameter states.

## Test

```bash
cargo test
npm run test:wasm
```
