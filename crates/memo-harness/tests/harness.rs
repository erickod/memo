use memo_harness::{BEGIN, END, HarnessError, InstallOpts, adapter, instruction_block};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

static COUNTER: AtomicU32 = AtomicU32::new(0);

struct Dir(PathBuf);

impl Dir {
    fn new() -> Self {
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let path = std::env::temp_dir().join(format!("memo-harness-{}-{n}", std::process::id()));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).unwrap();
        Dir(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }

    fn read(&self, rel: &str) -> String {
        fs::read_to_string(self.0.join(rel)).unwrap()
    }

    fn write(&self, rel: &str, text: &str) {
        let file = self.0.join(rel);
        fs::create_dir_all(file.parent().unwrap()).unwrap();
        fs::write(file, text).unwrap();
    }

    fn exists(&self, rel: &str) -> bool {
        self.0.join(rel).exists()
    }
}

impl Drop for Dir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

const PLAIN: InstallOpts = InstallOpts {
    mcp: false,
    print: false,
};
const WITH_MCP: InstallOpts = InstallOpts {
    mcp: true,
    print: false,
};

fn install(id: &str, dir: &Dir, opts: &InstallOpts) {
    adapter(id).unwrap().install(dir.path(), opts).unwrap();
}

/// GIVEN an empty repository
/// WHEN  the claude adapter installs without MCP
/// THEN  CLAUDE.md holds the instruction block between the memo markers and no .mcp.json exists
#[test]
fn test_given_empty_repo_when_claude_installed_then_claude_md_has_block_with_markers() {
    let dir = Dir::new();
    install("claude", &dir, &PLAIN);
    let text = dir.read("CLAUDE.md");
    assert!(
        text.starts_with(BEGIN) && text.trim_end().ends_with(END),
        "{text}"
    );
    assert!(text.contains(&instruction_block()));
    assert!(!dir.exists(".mcp.json"));
}

/// GIVEN a CLAUDE.md with user content
/// WHEN  the claude adapter installs
/// THEN  the user content is untouched and the block is appended after it
#[test]
fn test_given_existing_claude_md_when_installed_then_content_kept_and_block_appended() {
    let dir = Dir::new();
    dir.write("CLAUDE.md", "# Meu projeto\n\nRegras do time.\n");
    install("claude", &dir, &PLAIN);
    let text = dir.read("CLAUDE.md");
    assert!(
        text.starts_with("# Meu projeto\n\nRegras do time.\n"),
        "{text}"
    );
    assert!(text.contains(BEGIN) && text.contains(END));
}

/// GIVEN the block already installed
/// WHEN  the adapter installs a second time
/// THEN  the file is byte-identical, there is one begin marker and no change is reported (A13)
#[test]
fn test_given_block_installed_when_installed_again_then_file_unchanged() {
    let dir = Dir::new();
    install("claude", &dir, &WITH_MCP);
    let first = (dir.read("CLAUDE.md"), dir.read(".mcp.json"));
    let changes = adapter("claude")
        .unwrap()
        .install(dir.path(), &WITH_MCP)
        .unwrap();
    assert!(changes.iter().all(|c| !c.changed), "{changes:?}");
    assert_eq!((dir.read("CLAUDE.md"), dir.read(".mcp.json")), first);
    assert_eq!(dir.read("CLAUDE.md").matches(BEGIN).count(), 1);
}

/// GIVEN a CLAUDE.md whose block holds old text, with user text before and after it
/// WHEN  the adapter reinstalls
/// THEN  only what sits between the markers is replaced
#[test]
fn test_given_outdated_block_when_reinstalled_then_only_between_markers_replaced() {
    let dir = Dir::new();
    dir.write(
        "CLAUDE.md",
        &format!("antes\n{BEGIN}\ntexto velho\n{END}\ndepois\n"),
    );
    install("claude", &dir, &PLAIN);
    let text = dir.read("CLAUDE.md");
    assert!(
        text.starts_with("antes\n") && text.ends_with("depois\n"),
        "{text}"
    );
    assert!(!text.contains("texto velho") && text.contains("memo brief"));
}

/// GIVEN an empty repository and the print option
/// WHEN  the adapter installs with MCP
/// THEN  nothing is written, and the reported changes name CLAUDE.md and .mcp.json as changed
#[test]
fn test_given_print_when_installed_then_nothing_written_but_changes_reported() {
    let dir = Dir::new();
    let opts = InstallOpts {
        mcp: true,
        print: true,
    };
    let changes = adapter("claude")
        .unwrap()
        .install(dir.path(), &opts)
        .unwrap();
    assert!(!dir.exists("CLAUDE.md") && !dir.exists(".mcp.json"));
    let names: Vec<String> = changes
        .iter()
        .map(|c| c.path.file_name().unwrap().to_string_lossy().into_owned())
        .collect();
    assert_eq!(names, vec!["CLAUDE.md", ".mcp.json"]);
    assert!(changes.iter().all(|c| c.changed));
}

/// GIVEN a .mcp.json that already registers another server
/// WHEN  the claude adapter installs with MCP
/// THEN  the other server stays and 'memo' is a stdio server running 'memo serve'
#[test]
fn test_given_mcp_when_claude_installed_then_mcp_json_has_memo_server_and_keeps_others() {
    let dir = Dir::new();
    dir.write(".mcp.json", r#"{"mcpServers":{"other":{"command":"x"}}}"#);
    install("claude", &dir, &WITH_MCP);
    let json: serde_json::Value = serde_json::from_str(&dir.read(".mcp.json")).unwrap();
    assert_eq!(json["mcpServers"]["other"]["command"], "x");
    let memo = &json["mcpServers"]["memo"];
    assert_eq!(memo["type"], "stdio");
    assert_eq!(memo["command"], "memo");
    assert_eq!(memo["args"], serde_json::json!(["serve"]));
}

/// GIVEN a .mcp.json that is not valid JSON
/// WHEN  the adapter installs with MCP
/// THEN  it fails with Invalid and leaves both files untouched
#[test]
fn test_given_mcp_json_invalid_when_installed_then_error_and_file_untouched() {
    let dir = Dir::new();
    dir.write(".mcp.json", "{ not json");
    let err = adapter("claude")
        .unwrap()
        .install(dir.path(), &WITH_MCP)
        .err()
        .unwrap();
    assert!(matches!(err, HarnessError::Invalid { .. }), "{err:?}");
    assert_eq!(dir.read(".mcp.json"), "{ not json");
    assert!(!dir.exists("CLAUDE.md"));
}

/// GIVEN a repository using codex
/// WHEN  the codex adapter installs with MCP
/// THEN  AGENTS.md has the block and .codex/config.toml has an [mcp_servers.memo] table
#[test]
fn test_given_codex_when_installed_then_agents_md_block_and_codex_config_toml_block() {
    let dir = Dir::new();
    install("codex", &dir, &WITH_MCP);
    assert!(dir.read("AGENTS.md").contains(BEGIN));
    let toml = dir.read(".codex/config.toml");
    assert!(
        toml.contains("[mcp_servers.memo]") && toml.contains("command = \"memo\""),
        "{toml}"
    );
    assert!(toml.contains("args = [\"serve\"]"), "{toml}");
}

/// GIVEN a codex config.toml that already defines [mcp_servers.memo] by hand
/// WHEN  the codex adapter installs with MCP
/// THEN  it fails with Invalid instead of duplicating the table
#[test]
fn test_given_codex_table_by_hand_when_installed_then_conflict_error() {
    let dir = Dir::new();
    dir.write(
        ".codex/config.toml",
        "[mcp_servers.memo]\ncommand = \"other\"\n",
    );
    let err = adapter("codex")
        .unwrap()
        .install(dir.path(), &WITH_MCP)
        .err()
        .unwrap();
    assert!(matches!(err, HarnessError::Invalid { .. }), "{err:?}");
}

/// GIVEN claude installed with MCP over files holding user content
/// WHEN  uninstall runs
/// THEN  the block and the memo server are gone and the user content stays
#[test]
fn test_given_uninstall_when_run_then_block_and_mcp_entry_removed_other_content_kept() {
    let dir = Dir::new();
    dir.write("CLAUDE.md", "# Meu projeto\n");
    dir.write(".mcp.json", r#"{"mcpServers":{"other":{"command":"x"}}}"#);
    install("claude", &dir, &WITH_MCP);
    adapter("claude").unwrap().uninstall(dir.path()).unwrap();
    assert!(!dir.read("CLAUDE.md").contains(BEGIN));
    assert!(dir.read("CLAUDE.md").contains("# Meu projeto"));
    let json: serde_json::Value = serde_json::from_str(&dir.read(".mcp.json")).unwrap();
    assert!(json["mcpServers"].get("memo").is_none());
    assert_eq!(json["mcpServers"]["other"]["command"], "x");
}

/// GIVEN one repository with the integration installed and one without
/// WHEN  doctor runs on each
/// THEN  every check is ok in the first and the instructions check fails in the second
#[test]
fn test_given_doctor_when_installed_or_not_then_checks_reflect_state() {
    let (done, bare) = (Dir::new(), Dir::new());
    install("claude", &done, &WITH_MCP);
    let adapter = adapter("claude").unwrap();
    assert!(adapter.doctor(done.path()).iter().all(|c| c.ok));
    let checks = adapter.doctor(bare.path());
    assert!(
        checks.iter().any(|c| c.name == "instructions" && !c.ok),
        "{checks:?}"
    );
}

/// GIVEN the ids 'agy' and 'zed'
/// WHEN  adapters are requested
/// THEN  'agy' is Unsupported and 'zed' is Unknown; 'claude' and 'codex' exist
#[test]
fn test_given_agy_or_unknown_id_when_adapter_requested_then_clear_error() {
    assert!(matches!(
        adapter("agy").err().unwrap(),
        HarnessError::Unsupported(_)
    ));
    assert!(matches!(
        adapter("zed").err().unwrap(),
        HarnessError::Unknown(_)
    ));
    assert_eq!(adapter("claude").unwrap().id(), "claude");
    assert_eq!(adapter("codex").unwrap().id(), "codex");
}

/// GIVEN the instruction block
/// WHEN  its text is read
/// THEN  it carries the fixed rules of the spec: brief first, impact before changes, '~', notes, --task
#[test]
fn test_given_block_text_when_read_then_matches_spec_instructions() {
    let block = instruction_block();
    assert!(block.starts_with(BEGIN) && block.trim_end().ends_with(END));
    for needle in [
        "memo brief",
        "memo query impact",
        "`~`",
        "memo note add",
        "--task <id>",
    ] {
        assert!(block.contains(needle), "missing {needle}");
    }
}

/// GIVEN a repository where only AGENTS.md exists
/// WHEN  detect runs for codex and claude
/// THEN  codex is detected and claude is not
#[test]
fn test_given_only_agents_md_when_detect_then_codex_only() {
    let dir = Dir::new();
    dir.write("AGENTS.md", "# x\n");
    assert!(adapter("codex").unwrap().detect(dir.path()));
    assert!(!adapter("claude").unwrap().detect(dir.path()));
}

/// GIVEN the instruction block written by 'memo init'
/// WHEN  it is read
/// THEN  it recommends reading the 'ler:' spots before querying again (TASK-7, D4)
#[test]
fn test_given_instruction_block_when_read_then_recommends_reading() {
    assert!(instruction_block().contains("ler:"));
}
