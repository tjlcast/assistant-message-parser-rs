use assistant_message_parser::{AssistantMessageParser, ContentBlock};

fn test_parser() -> AssistantMessageParser {
    let tools = [
        ("read_file", ["path", "start_line", "end_line"].as_slice()),
        (
            "write_to_file",
            ["path", "content", "line_count"].as_slice(),
        ),
        ("browser_action", [].as_slice()),
        ("search_files", ["regex", "path"].as_slice()),
        ("execute_command", ["command"].as_slice()),
        (
            "ask_followup_question",
            ["question", "follow_up"].as_slice(),
        ),
        ("new_rule", [].as_slice()),
        ("echo", ["message"].as_slice()),
    ];

    let tool_names = tools
        .iter()
        .map(|(name, _)| (*name).to_owned())
        .collect::<Vec<_>>();
    let tool_param_names = tools
        .iter()
        .flat_map(|(_, params)| params.iter().map(|param| (*param).to_owned()))
        .collect::<Vec<_>>();

    AssistantMessageParser::new(Some(tool_names), Some(tool_param_names))
}

fn non_empty(block: &ContentBlock) -> bool {
    !matches!(block, ContentBlock::Text(text) if text.content.is_empty())
}

#[test]
fn split_inside_tool_opening_tag_middle() {
    let mut parser = test_parser();
    let _ = parser.process_chunk("First: <rea").unwrap();
    let _ = parser.process_chunk("d_file><path>file1.ts</path></read_file>")
        .unwrap();
    parser
        .process_chunk("Second: <read_file><path>file2.ts</path></read_file>")
        .unwrap();
    parser.finalize_content_blocks();

    let blocks: Vec<_> = parser.get_content_blocks().into_iter().filter(non_empty).collect();
    assert_eq!(blocks.len(), 4);
    assert!(matches!(&blocks[0], ContentBlock::Text(text) if text.content == "First:"));
    assert!(matches!(&blocks[1], ContentBlock::ToolUse(tool) if tool.params.get("path").unwrap() == "file1.ts"));
    assert!(matches!(&blocks[2], ContentBlock::Text(text) if text.content == "Second:"));
    assert!(matches!(&blocks[3], ContentBlock::ToolUse(tool) if tool.params.get("path").unwrap() == "file2.ts"));
}

#[test]
fn split_inside_tool_opening_tag_right_before_gt() {
    let mut parser = test_parser();
    let _ = parser.process_chunk("First: <read_file").unwrap();
    let _ = parser.process_chunk("><path>file1.ts</path></read_file>")
        .unwrap();
    parser
        .process_chunk("Second: <read_file><path>file2.ts</path></read_file>")
        .unwrap();
    parser.finalize_content_blocks();

    let blocks: Vec<_> = parser.get_content_blocks().into_iter().filter(non_empty).collect();
    assert_eq!(blocks.len(), 4);
    assert!(matches!(&blocks[0], ContentBlock::Text(text) if text.content == "First:"));
    assert!(matches!(&blocks[1], ContentBlock::ToolUse(tool) if tool.params.get("path").unwrap() == "file1.ts"));
    assert!(matches!(&blocks[2], ContentBlock::Text(text) if text.content == "Second:"));
    assert!(matches!(&blocks[3], ContentBlock::ToolUse(tool) if tool.params.get("path").unwrap() == "file2.ts"));
}

#[test]
fn split_inside_tool_closing_tag() {
    let mut parser = test_parser();
    let _ = parser.process_chunk("First: <read_file><path>file1.ts</path></read_")
        .unwrap();
    let _ = parser.process_chunk("file>").unwrap();
    parser
        .process_chunk("Second: <read_file><path>file2.ts</path></read_file>")
        .unwrap();
    parser.finalize_content_blocks();

    let blocks: Vec<_> = parser.get_content_blocks().into_iter().filter(non_empty).collect();
    assert_eq!(blocks.len(), 4);
    assert!(matches!(&blocks[1], ContentBlock::ToolUse(tool) if tool.params.get("path").unwrap() == "file1.ts" && !tool.partial));
    assert!(matches!(&blocks[3], ContentBlock::ToolUse(tool) if tool.params.get("path").unwrap() == "file2.ts" && !tool.partial));
}

#[test]
fn split_inside_param_opening_tag() {
    let mut parser = test_parser();
    let _ = parser.process_chunk("First: <read_file><pa").unwrap();
    let _ = parser.process_chunk("th>file1.ts</path></read_file>")
        .unwrap();
    parser
        .process_chunk("Second: <read_file><path>file2.ts</path></read_file>")
        .unwrap();
    parser.finalize_content_blocks();

    let blocks: Vec<_> = parser.get_content_blocks().into_iter().filter(non_empty).collect();
    assert!(matches!(&blocks[1], ContentBlock::ToolUse(tool) if tool.params.get("path").unwrap() == "file1.ts"));
    assert!(matches!(&blocks[3], ContentBlock::ToolUse(tool) if tool.params.get("path").unwrap() == "file2.ts"));
}

#[test]
fn split_inside_param_closing_tag() {
    let mut parser = test_parser();
    let _ = parser.process_chunk("First: <read_file><path>file1.ts</pa").unwrap();
    let _ = parser.process_chunk("th></read_file>").unwrap();
    parser
        .process_chunk("Second: <read_file><path>file2.ts</path></read_file>")
        .unwrap();
    parser.finalize_content_blocks();

    let blocks: Vec<_> = parser.get_content_blocks().into_iter().filter(non_empty).collect();
    assert!(matches!(&blocks[1], ContentBlock::ToolUse(tool) if tool.params.get("path").unwrap() == "file1.ts" && !tool.partial));
}

#[test]
fn split_inside_filename() {
    let mut parser = test_parser();
    let _ = parser
        .process_chunk("First: <read_file><path>file")
        .unwrap();
    let _ = parser
        .process_chunk("1.ts</path></read_file>")
        .unwrap();
    parser
        .process_chunk("Second: <read_file><path>file2.ts</path></read_file>")
        .unwrap();
    parser.finalize_content_blocks();

    let blocks: Vec<_> = parser.get_content_blocks().into_iter().filter(non_empty).collect();
    assert!(matches!(&blocks[1], ContentBlock::ToolUse(tool) if tool.params.get("path").unwrap() == "file1.ts"));
}

#[test]
fn split_at_every_character_position() {
    let mut parser = test_parser();
    let msg = "A <read_file><path>x.ts</path></read_file> B <read_file><path>y.ts</path></read_file>";
    for ch in msg.chars() {
        parser.process_chunk(&ch.to_string()).unwrap();
    }
    parser.finalize_content_blocks();

    let blocks: Vec<_> = parser.get_content_blocks().into_iter().filter(non_empty).collect();
    assert_eq!(blocks.len(), 4);
    assert!(matches!(&blocks[0], ContentBlock::Text(text) if text.content == "A"));
    assert!(matches!(&blocks[1], ContentBlock::ToolUse(tool) if tool.params.get("path").unwrap() == "x.ts" && !tool.partial));
    assert!(matches!(&blocks[2], ContentBlock::Text(text) if text.content == "B"));
    assert!(matches!(&blocks[3], ContentBlock::ToolUse(tool) if tool.params.get("path").unwrap() == "y.ts" && !tool.partial));
}

#[test]
fn split_between_text_and_tool_boundary_single_char() {
    let mut parser = test_parser();
    let _ = parser.process_chunk("Hello").unwrap();
    let _ = parser.process_chunk("<").unwrap();
    let _ = parser.process_chunk("read_file><path>f</path></read_file>")
        .unwrap();
    parser.finalize_content_blocks();

    let blocks: Vec<_> = parser.get_content_blocks().into_iter().filter(non_empty).collect();
    assert_eq!(blocks.len(), 2);
    assert!(matches!(&blocks[0], ContentBlock::Text(text) if text.content == "Hello"));
    assert!(matches!(&blocks[1], ContentBlock::ToolUse(tool) if tool.params.get("path").unwrap() == "f"));
}

#[test]
fn split_right_after_lt_then_rest_of_tag() {
    let mut parser = test_parser();
    let _ = parser.process_chunk("Hello <").unwrap();
    let _ = parser.process_chunk("read_file><path>f</path></read_file>")
        .unwrap();
    parser.finalize_content_blocks();

    let blocks: Vec<_> = parser.get_content_blocks().into_iter().filter(non_empty).collect();
    assert_eq!(blocks.len(), 2);
    assert!(matches!(&blocks[0], ContentBlock::Text(text) if text.content == "Hello"));
    assert!(matches!(&blocks[1], ContentBlock::ToolUse(tool) if tool.params.get("path").unwrap() == "f"));
}

#[test]
fn split_right_before_lt_of_tool_start_tag() {
    let mut parser = test_parser();
    let _ = parser.process_chunk("First:").unwrap();
    let _ = parser.process_chunk(" <read_file><path>f</path></read_file>")
        .unwrap();
    parser.finalize_content_blocks();

    let blocks: Vec<_> = parser.get_content_blocks().into_iter().filter(non_empty).collect();
    assert_eq!(blocks.len(), 2);
    assert!(matches!(&blocks[0], ContentBlock::Text(text) if text.content == "First:"));
    assert!(matches!(&blocks[1], ContentBlock::ToolUse(tool) if tool.params.get("path").unwrap() == "f"));
}

#[test]
fn split_multiple_params_each_different_cut() {
    let mut parser = test_parser();
    let _ = parser.process_chunk("<read_fi").unwrap();
    let _ = parser.process_chunk("le><path>src/f").unwrap();
    let _ = parser.process_chunk("ile.ts</path><start_lin").unwrap();
    let _ = parser.process_chunk("e>10</start_line><end_").unwrap();
    let _ = parser.process_chunk("line>20</end_line></read_").unwrap();
    parser.process_chunk("file>").unwrap();
    parser.finalize_content_blocks();

    let blocks: Vec<_> = parser.get_content_blocks().into_iter().filter(non_empty).collect();
    assert_eq!(blocks.len(), 1);
    let ContentBlock::ToolUse(tool) = &blocks[0] else {
        panic!("expected tool");
    };
    assert_eq!(tool.params.get("path").unwrap(), "src/file.ts");
    assert_eq!(tool.params.get("start_line").unwrap(), "10");
    assert_eq!(tool.params.get("end_line").unwrap(), "20");
    assert!(!tool.partial);
}

#[test]
fn split_write_to_file_content_across_chunks() {
    let mut parser = test_parser();
    let _ = parser.process_chunk("<write_to_").unwrap();
    let _ = parser.process_chunk("file><path>out.ts</path><co").unwrap();
    let _ = parser.process_chunk("ntent>\nfunction hello() {\n  return ").unwrap();
    let _ = parser.process_chunk("42;\n}\n</content><li").unwrap();
    let _ = parser.process_chunk("ne_count>3</line_count></write_to_").unwrap();
    parser.process_chunk("file>").unwrap();
    parser.finalize_content_blocks();

    let blocks: Vec<_> = parser.get_content_blocks().into_iter().filter(non_empty).collect();
    assert_eq!(blocks.len(), 1);
    let ContentBlock::ToolUse(tool) = &blocks[0] else {
        panic!("expected tool");
    };
    assert_eq!(tool.params.get("path").unwrap(), "out.ts");
    assert_eq!(tool.params.get("line_count").unwrap(), "3");
    let content = tool.params.get("content").unwrap();
    assert!(content.contains("function hello()"));
    assert!(content.contains("return 42;"));
    assert!(!tool.partial);
}

#[test]
fn split_echo_tool_every_two_chars() {
    let mut parser = test_parser();
    let msg = "Say: <echo><message>hi!</message></echo>";
    let chars: Vec<char> = msg.chars().collect();
    for pair in chars.chunks(2) {
        let chunk: String = pair.iter().collect();
        parser.process_chunk(&chunk).unwrap();
    }
    parser.finalize_content_blocks();

    let blocks: Vec<_> = parser.get_content_blocks().into_iter().filter(non_empty).collect();
    assert_eq!(blocks.len(), 2);
    assert!(matches!(&blocks[0], ContentBlock::Text(text) if text.content == "Say:"));
    let ContentBlock::ToolUse(tool) = &blocks[1] else {
        panic!("expected tool");
    };
    assert_eq!(tool.name, "echo");
    assert_eq!(tool.params.get("message").unwrap(), "hi!");
    assert!(!tool.partial);
}

#[test]
fn split_execute_command_between_cwd_then_cmd_tag() {
    let mut parser = test_parser();
    let _ = parser.process_chunk("<exe").unwrap();
    let _ = parser.process_chunk("cute_command><comman").unwrap();
    let _ = parser.process_chunk("d>ls -la</comman").unwrap();
    let _ = parser.process_chunk("d></execute_comman").unwrap();
    parser.process_chunk("d>").unwrap();
    parser.finalize_content_blocks();

    let blocks: Vec<_> = parser.get_content_blocks().into_iter().filter(non_empty).collect();
    assert_eq!(blocks.len(), 1);
    let ContentBlock::ToolUse(tool) = &blocks[0] else {
        panic!("expected tool");
    };
    assert_eq!(tool.name, "execute_command");
    assert_eq!(tool.params.get("command").unwrap(), "ls -la");
    assert!(!tool.partial);
}

#[test]
fn split_multiple_tool_types_with_tricky_cuts() {
    let mut parser = test_parser();
    let _ = parser.process_chunk("Pre ").unwrap();
    let _ = parser.process_chunk("<read").unwrap();
    let _ = parser.process_chunk("_file><pa").unwrap();
    let _ = parser.process_chunk("th>a.ts</pa").unwrap();
    let _ = parser.process_chunk("th></read_file").unwrap();
    let _ = parser.process_chunk("> Mid ").unwrap();
    let _ = parser.process_chunk("<search_files><").unwrap();
    let _ = parser.process_chunk("regex>foo</re").unwrap();
    let _ = parser.process_chunk("gex><path>src</p").unwrap();
    let _ = parser.process_chunk("ath></search_fil").unwrap();
    let _ = parser.process_chunk("es> Post").unwrap();
    parser.finalize_content_blocks();

    let blocks: Vec<_> = parser.get_content_blocks().into_iter().filter(non_empty).collect();
    assert_eq!(blocks.len(), 5);
    assert!(matches!(&blocks[0], ContentBlock::Text(text) if text.content == "Pre"));
    assert!(matches!(&blocks[1], ContentBlock::ToolUse(tool) if tool.name == "read_file" && tool.params.get("path").unwrap() == "a.ts"));
    assert!(matches!(&blocks[2], ContentBlock::Text(text) if text.content == "Mid"));
    assert!(matches!(&blocks[3], ContentBlock::ToolUse(tool) if tool.name == "search_files" && tool.params.get("regex").unwrap() == "foo" && tool.params.get("path").unwrap() == "src"));
    assert!(matches!(&blocks[4], ContentBlock::Text(text) if text.content == "Post"));
}

#[test]
fn split_search_files_xml_like_param_across_chunks() {
    let mut parser = test_parser();
    let _ = parser.process_chunk("<search_f").unwrap();
    let _ = parser.process_chunk("iles><regex><d").unwrap();
    let _ = parser.process_chunk("iv>.*</div></reg").unwrap();
    let _ = parser.process_chunk("ex><path>src</").unwrap();
    let _ = parser.process_chunk("path></search_f").unwrap();
    parser.process_chunk("iles>").unwrap();
    parser.finalize_content_blocks();

    let blocks: Vec<_> = parser.get_content_blocks().into_iter().filter(non_empty).collect();
    assert_eq!(blocks.len(), 1);
    let ContentBlock::ToolUse(tool) = &blocks[0] else {
        panic!("expected tool");
    };
    assert_eq!(tool.name, "search_files");
    assert_eq!(tool.params.get("regex").unwrap(), "<div>.*</div>");
    assert_eq!(tool.params.get("path").unwrap(), "src");
    assert!(!tool.partial);
}

#[test]
fn split_ask_followup_question_many_cuts() {
    let mut parser = test_parser();
    let _ = parser.process_chunk("Q: <ask_follo").unwrap();
    let _ = parser.process_chunk("wup_question><ques").unwrap();
    let _ = parser.process_chunk("tion>What?</ques").unwrap();
    let _ = parser.process_chunk("tion><follo").unwrap();
    let _ = parser.process_chunk("w_up>yes</follow_u").unwrap();
    let _ = parser.process_chunk("p></ask_foll").unwrap();
    parser.process_chunk("owup_question>").unwrap();
    parser.finalize_content_blocks();

    let blocks: Vec<_> = parser.get_content_blocks().into_iter().filter(non_empty).collect();
    assert_eq!(blocks.len(), 2);
    assert!(matches!(&blocks[0], ContentBlock::Text(text) if text.content == "Q:"));
    let ContentBlock::ToolUse(tool) = &blocks[1] else {
        panic!("expected tool");
    };
    assert_eq!(tool.name, "ask_followup_question");
    assert_eq!(tool.params.get("question").unwrap(), "What?");
    assert_eq!(tool.params.get("follow_up").unwrap(), "yes");
    assert!(!tool.partial);
}

#[test]
fn split_tool_use_with_no_params_tricky_cut() {
    let mut parser = test_parser();
    let _ = parser.process_chunk("<browser_act").unwrap();
    let _ = parser.process_chunk("ion></bro").unwrap();
    parser.process_chunk("wser_action>").unwrap();
    parser.finalize_content_blocks();

    let blocks: Vec<_> = parser.get_content_blocks().into_iter().filter(non_empty).collect();
    assert_eq!(blocks.len(), 1);
    let ContentBlock::ToolUse(tool) = &blocks[0] else {
        panic!("expected tool");
    };
    assert_eq!(tool.name, "browser_action");
    assert!(tool.params.is_empty());
    assert!(!tool.partial);
}

#[test]
fn split_consecutive_tools_tricky_cuts() {
    let mut parser = test_parser();
    let _ = parser.process_chunk("<read_fi").unwrap();
    let _ = parser.process_chunk("le><path>a</path></read_").unwrap();
    let _ = parser.process_chunk("file><read_").unwrap();
    let _ = parser.process_chunk("file><path>b</path></r").unwrap();
    parser.process_chunk("ead_file>").unwrap();
    parser.finalize_content_blocks();

    let blocks: Vec<_> = parser.get_content_blocks().into_iter().filter(non_empty).collect();
    assert_eq!(blocks.len(), 2);
    assert!(matches!(&blocks[0], ContentBlock::ToolUse(tool) if tool.params.get("path").unwrap() == "a" && !tool.partial));
    assert!(matches!(&blocks[1], ContentBlock::ToolUse(tool) if tool.params.get("path").unwrap() == "b" && !tool.partial));
}

#[test]
fn split_text_containing_partial_tag_like_prefix() {
    let mut parser = test_parser();
    let _ = parser.process_chunk("abc <read_fil").unwrap();
    let _ = parser.process_chunk("e xyz").unwrap();
    parser.finalize_content_blocks();

    let blocks: Vec<_> = parser.get_content_blocks().into_iter().filter(non_empty).collect();
    assert_eq!(blocks.len(), 1);
    assert!(matches!(&blocks[0], ContentBlock::Text(text) if text.content == "abc <read_file xyz"));
}

#[test]
fn split_unclosed_tool_with_progressive_param_sending() {
    let mut parser = test_parser();
    let _ = parser.process_chunk("<read_file><pa").unwrap();
    let blocks = parser.process_chunk("th>incomplete").unwrap();

    let tool = blocks.iter().find_map(|b| match b {
        ContentBlock::ToolUse(t) => Some(t),
        _ => None,
    }).unwrap();
    assert_eq!(tool.params.get("path").unwrap(), "incomplete");
    assert!(tool.partial);
}

#[test]
fn split_complex_message_across_maximally_tricky_boundaries() {
    let mut parser = test_parser();

    parser.process_chunk("I'll").unwrap();
    parser.process_chunk(" help you with that task.\n").unwrap();
    parser.process_chunk("\n<re").unwrap();
    parser.process_chunk("ad_file><pat").unwrap();
    parser.process_chunk("h>src/index.ts</pa").unwrap();
    parser.process_chunk("th></read_file>\n").unwrap();
    parser.process_chunk("\nNow let").unwrap();
    parser.process_chunk("'s modify the f").unwrap();
    parser.process_chunk("ile:\n\n<wr").unwrap();
    parser.process_chunk("ite_to_file><path>src/i").unwrap();
    parser.process_chunk("ndex.ts</path><con").unwrap();
    parser.process_chunk("tent>\nconsole.log(4").unwrap();
    parser.process_chunk("2);\n</content><").unwrap();
    parser.process_chunk("line_count>1</line_c").unwrap();
    parser.process_chunk("ount></write_to_f").unwrap();
    parser.process_chunk("ile>\n\n").unwrap();
    parser.process_chunk("Done.").unwrap();
    parser.finalize_content_blocks();

    let blocks: Vec<_> = parser.get_content_blocks().into_iter().filter(non_empty).collect();
    assert_eq!(blocks.len(), 5);
    assert!(matches!(&blocks[0], ContentBlock::Text(text) if text.content == "I'll help you with that task."));
    assert!(matches!(&blocks[1], ContentBlock::ToolUse(tool) if tool.name == "read_file"));
    assert!(matches!(&blocks[2], ContentBlock::Text(text) if text.content == "Now let's modify the file:"));
    assert!(matches!(&blocks[3], ContentBlock::ToolUse(tool) if tool.name == "write_to_file"));
    assert!(matches!(&blocks[4], ContentBlock::Text(text) if text.content == "Done."));
}
