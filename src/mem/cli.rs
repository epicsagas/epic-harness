//! cli.rs — CLI subcommand parsing + dispatch

use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::PathBuf;

use uuid::Uuid;

use super::graph::{rebuild_graph, related_nodes};
use super::store::{
    Edge, IndexNode, Node, NodeFrontmatter, append_edge, delete_node_file, importance_for_type,
    list_node_ids, now_iso, parse_node, query_nodes, read_node, remove_edges_for_node,
    remove_from_index, search_nodes, serialize_node, smart_recall, upsert_index, validate_uuid,
    write_node, write_node_dedup,
};

const SUBCOMMANDS: &[(&str, &str)] = &[
    ("add", "Add a new memory node"),
    ("edit", "Edit an existing node"),
    ("remove", "Remove a node and its edges"),
    ("delete", "[deprecated] Use 'remove' instead"),
    ("list", "List/filter nodes from the index"),
    ("query", "[deprecated] Use 'list' instead"),
    ("search", "Full-text search across node files"),
    ("related", "BFS traversal — find related nodes"),
    ("link", "Create a directed edge between two nodes"),
    ("graph", "Manage the graph cache (rebuild)"),
    ("validate", "Check all node files for parse errors"),
    (
        "export",
        "Dump all nodes to Markdown files (for Git backup)",
    ),
    ("migrate", "Import legacy project memory files"),
    ("context", "Show recently-updated nodes for a project"),
    (
        "recall",
        "Smart recall — relevance-ranked memories for current task",
    ),
    ("mcp", "Run as stdio MCP server (JSON-RPC 2.0)"),
    (
        "mcp-install",
        "Register the harness-mem MCP server in Claude Code",
    ),
    ("serve", "Start the REST + Web UI server"),
    ("help", "Show this help message"),
];

fn print_help() {
    println!("harness mem — Cross-Agent Unified Memory\n");
    println!("USAGE:");
    println!("  harness mem <SUBCOMMAND> [OPTIONS]\n");
    println!("SUBCOMMANDS:");
    for (name, desc) in SUBCOMMANDS {
        println!("  {name:<14} {desc}");
    }
    println!("\nRun 'harness mem <SUBCOMMAND> --help' for subcommand-specific options.");
}

fn print_subcommand_help(sub: &str) {
    match sub {
        "add" => {
            println!("harness mem add — Add a new memory node\n");
            println!("USAGE:");
            println!("  harness mem add [OPTIONS]\n");
            println!("OPTIONS:");
            println!("  --title <text>      Node title (default: Untitled)");
            println!(
                "  --type <type>       Node type: concept|decision|pattern|task|... (default: concept)"
            );
            println!("  --tags <a,b,c>      Comma-separated tags");
            println!("  --project <name>    Associate with a project slug");
            println!("  --agent <name>      Associate with an agent name");
            println!("  --body <text>       Node body content");
            println!("  --importance <0-1>  Importance score (auto-set by type if omitted)");
            println!("\nOUTPUT: {{\"id\":\"<uuid>\"}}");
        }
        "edit" => {
            println!("harness mem edit — Edit an existing node\n");
            println!("USAGE:");
            println!("  harness mem edit <ID> [OPTIONS]\n");
            println!("OPTIONS:");
            println!("  --title <text>      New title");
            println!("  --type <type>       New node type");
            println!("  --tags <a,b,c>      Replace tags (comma-separated)");
            println!("  --body <text>       Replace body content");
            println!("\nOUTPUT: {{\"id\":\"<uuid>\"}}");
        }
        "remove" | "delete" => {
            println!("harness mem remove — Remove a node and its edges\n");
            println!("USAGE:");
            println!("  harness mem remove <ID>\n");
            println!("OUTPUT: {{\"deleted\":\"<uuid>\"}}");
        }
        "list" | "query" => {
            println!("harness mem list — List/filter nodes from the index\n");
            println!("USAGE:");
            println!("  harness mem list [OPTIONS]\n");
            println!("OPTIONS:");
            println!("  --tag <tag>         Filter by tag");
            println!("  --type <type>       Filter by node type");
            println!("  --project <name>    Filter by project slug");
            println!("  --agent <name>      Filter by agent name");
            println!("\nOUTPUT: JSON array of matching index nodes");
        }
        "search" => {
            println!("harness mem search — Full-text search across node files\n");
            println!("USAGE:");
            println!("  harness mem search <QUERY>\n");
            println!("  Uses ripgrep (rg) with grep fallback.");
            println!("\nOUTPUT: matching lines (file:line:content)");
        }
        "related" => {
            println!("harness mem related — BFS traversal to find related nodes\n");
            println!("USAGE:");
            println!("  harness mem related <ID> [OPTIONS]\n");
            println!("OPTIONS:");
            println!("  --depth <n>         Max traversal hops (default: 2)");
            println!("\nOUTPUT: JSON array of related node IDs");
        }
        "link" => {
            println!("harness mem link — Create a directed edge between two nodes\n");
            println!("USAGE:");
            println!("  harness mem link <SRC-ID> <DST-ID> [OPTIONS]\n");
            println!("OPTIONS:");
            println!("  --relation <name>   Edge label (default: related)");
            println!("  --weight <float>    Edge weight (default: 1.0)");
            println!("\nOUTPUT: {{\"edge_id\":\"<uuid>\"}}");
        }
        "graph" => {
            println!("harness mem graph — Manage the graph cache\n");
            println!("USAGE:");
            println!("  harness mem graph rebuild\n");
            println!("  Rebuilds graph.json from current nodes + edges.");
            println!("\nOUTPUT: {{\"status\":\"ok\"}}");
        }
        "validate" => {
            println!("harness mem validate — Check all node files for parse errors\n");
            println!("USAGE:");
            println!("  harness mem validate\n");
            println!("OUTPUT: JSON array of {{\"file\", \"error\"}} — empty array if all valid.");
            println!("EXIT:   0 if valid, 1 if any errors found");
        }
        "export" => {
            println!("harness mem export — Dump all nodes to Markdown files\n");
            println!("USAGE:");
            println!("  harness mem export [OPTIONS]\n");
            println!("Exports all nodes from the SQLite DB to ~/.harness/exports/<id>.md");
            println!("Suitable for Git backup and diffing.\n");
            println!("OPTIONS:");
            println!("  --out <dir>         Output directory (default: ~/.harness/exports)");
            println!("  --dry-run           Preview without writing");
            println!("\nOUTPUT: {{\"exported\":<n>, \"dir\":\"<path>\"}}");
        }
        "migrate" => {
            println!("harness mem migrate — Import legacy project memory files\n");
            println!("USAGE:");
            println!("  harness mem migrate [OPTIONS]\n");
            println!("OPTIONS:");
            println!("  --project <slug>    Migrate only this project (default: all)");
            println!("  --all               Migrate all projects");
            println!("  --dry-run           Preview without writing");
            println!("\nOUTPUT: {{\"migrated\":<n>, \"dry_run\":<bool>, \"nodes\":[...]}}");
        }
        "context" => {
            println!("harness mem context — Show recently-updated nodes\n");
            println!("USAGE:");
            println!("  harness mem context [OPTIONS]\n");
            println!("OPTIONS:");
            println!("  --project <name>    Filter by project slug");
            println!("  --limit <n>         Max nodes to return (default: 5)");
            println!("\nOUTPUT: JSON array of index nodes sorted by updated desc");
        }
        "recall" => {
            println!("harness mem recall — Smart contextual recall\n");
            println!("USAGE:");
            println!("  harness mem recall <HINT> [OPTIONS]\n");
            println!("ARGUMENTS:");
            println!(
                "  <HINT>              Describe current task (e.g. 'auth refactor', 'CI fix')\n"
            );
            println!("OPTIONS:");
            println!("  --project <name>    Filter by project slug");
            println!("  --limit <n>         Max nodes to return (default: 10)");
            println!("\nOUTPUT: JSON array of relevance-scored nodes");
            println!(
                "\nScoring: recency(25%) + importance(35%) + access_freq(15%) + FTS_match(25%)"
            );
        }
        "mcp-install" => {
            println!("harness mem mcp-install — Register the harness-mem MCP server\n");
            println!("USAGE:");
            println!("  harness mem mcp-install [OPTIONS]\n");
            println!("Registers `epic-harness mem mcp` as mcpServers.harness-mem in");
            println!("~/.claude.json. No Node.js or external files needed.\n");
            println!("OPTIONS:");
            println!("  --force             Overwrite an existing harness-mem registration");
            println!("  --dry-run           Preview without writing ~/.claude.json");
        }
        "serve" => {
            println!("harness mem serve — Start the REST + Web UI server\n");
            println!("USAGE:");
            println!("  harness mem serve [OPTIONS]\n");
            println!("OPTIONS:");
            println!("  --port <n>          Port to listen on (default: 7700)");
            println!("\n  Web UI: http://localhost:7700");
            println!("  API:    http://localhost:7700/api/nodes");
        }
        _ => print_help(),
    }
}

/// Levenshtein distance for "did you mean?" suggestions (max checked distance: 3)
fn levenshtein(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.iter().enumerate() {
        let mut curr = vec![0usize; b.len() + 1];
        curr[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            curr[j + 1] = if ca == cb {
                prev[j]
            } else {
                1 + prev[j + 1].min(curr[j]).min(prev[j])
            };
        }
        prev = curr;
    }
    prev[b.len()]
}

pub fn dispatch(args: &[String]) -> i32 {
    let sub = match args.first().map(|s| s.as_str()) {
        Some(s) => s,
        None => {
            print_help();
            return 0; // help is not an error
        }
    };

    // --help / -h on any subcommand
    if args.get(1).map(|s| s.as_str()) == Some("--help")
        || args.get(1).map(|s| s.as_str()) == Some("-h")
    {
        print_subcommand_help(sub);
        return 0;
    }

    let result = match sub {
        "add" => cmd_add(&args[1..]),
        "edit" => cmd_edit(&args[1..]),
        "remove" | "delete" => {
            if sub == "delete" {
                eprintln!("[deprecated] 'delete' is deprecated, use 'remove' instead.");
            }
            cmd_delete(&args[1..])
        }
        "list" | "query" => {
            if sub == "query" {
                eprintln!("[deprecated] 'query' is deprecated, use 'list' instead.");
            }
            cmd_query(&args[1..])
        }
        "search" => cmd_search(&args[1..]),
        "related" => cmd_related(&args[1..]),
        "link" => cmd_link(&args[1..]),
        "graph" => cmd_graph(&args[1..]),
        "validate" => cmd_validate(),
        "export" => cmd_export(&args[1..]),
        "migrate" => cmd_migrate(&args[1..]),
        "context" => cmd_context(&args[1..]),
        "recall" => cmd_recall(&args[1..]),
        "mcp" => return super::mcp::run_mcp_server(),
        "mcp-install" => cmd_mcp_install(&args[1..]),
        "serve" => return super::server::serve(&args[1..]),
        "help" | "--help" | "-h" => {
            print_help();
            return 0;
        }
        _ => {
            // "did you mean?" suggestion
            let known: Vec<&str> = SUBCOMMANDS.iter().map(|(n, _)| *n).collect();
            let best = known
                .iter()
                .filter_map(|&name| {
                    let d = levenshtein(sub, name);
                    if d <= 3 { Some((d, name)) } else { None }
                })
                .min_by_key(|(d, _)| *d);
            eprintln!("error: unknown subcommand '{sub}'");
            if let Some((_, suggestion)) = best {
                eprintln!("       did you mean '{suggestion}'?");
            }
            eprintln!("\nRun 'harness mem help' for available subcommands.");
            return 1;
        }
    };

    match result {
        Ok(code) => code,
        Err(e) => {
            eprintln!("error: {e}");
            1
        }
    }
}

// ── Helpers ───────────────────────────────────────────

fn parse_flags(args: &[String]) -> (Vec<String>, HashMap<String, String>) {
    let mut positional = vec![];
    let mut flags: HashMap<String, String> = HashMap::new();
    let mut i = 0;
    while i < args.len() {
        if args[i].starts_with("--") {
            let key = args[i].trim_start_matches('-').to_string();
            let val = args.get(i + 1).cloned().unwrap_or_default();
            flags.insert(key, val);
            i += 2;
        } else {
            positional.push(args[i].clone());
            i += 1;
        }
    }
    (positional, flags)
}

fn csv_to_vec(s: &str) -> Vec<String> {
    s.split(',')
        .map(|t| t.trim().to_string())
        .filter(|t| !t.is_empty())
        .collect()
}

// ── Commands ──────────────────────────────────────────

fn cmd_add(args: &[String]) -> io::Result<i32> {
    let (_, flags) = parse_flags(args);

    let title = flags
        .get("title")
        .cloned()
        .unwrap_or_else(|| "Untitled".to_string());
    let node_type = flags
        .get("type")
        .cloned()
        .unwrap_or_else(|| "concept".to_string());
    let tags = csv_to_vec(flags.get("tags").map(|s| s.as_str()).unwrap_or(""));
    let projects = csv_to_vec(flags.get("project").map(|s| s.as_str()).unwrap_or(""));
    let agents = csv_to_vec(flags.get("agent").map(|s| s.as_str()).unwrap_or(""));
    let body = flags.get("body").cloned().unwrap_or_default();
    let importance = flags
        .get("importance")
        .and_then(|v| v.parse::<f64>().ok())
        .unwrap_or_else(|| importance_for_type(&node_type))
        .clamp(0.0, 1.0);

    let id = Uuid::new_v4().to_string();
    let now = now_iso();

    let node = Node {
        frontmatter: NodeFrontmatter {
            id: id.clone(),
            node_type,
            title,
            tags,
            projects,
            agents,
            created: now.clone(),
            updated: now,
            importance,
            access_count: 0,
            accessed_at: String::new(),
        },
        body,
    };

    // Single DB connection: dedup check + write in one open_db() call
    match write_node_dedup(&node, 24)? {
        (existing_id, true) => println!("{{\"id\":\"{existing_id}\",\"deduplicated\":true}}"),
        (_, false) => println!("{{\"id\":\"{id}\"}}"),
    }
    Ok(0)
}

fn cmd_edit(args: &[String]) -> io::Result<i32> {
    let (pos, flags) = parse_flags(args);
    let id = pos
        .first()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "edit requires <id>"))?;
    if !validate_uuid(id) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "invalid node id",
        ));
    }

    let mut node = read_node(id)?;

    if let Some(title) = flags.get("title") {
        node.frontmatter.title = title.clone();
    }
    if let Some(t) = flags.get("type") {
        node.frontmatter.node_type = t.clone();
    }
    if let Some(tags) = flags.get("tags") {
        node.frontmatter.tags = csv_to_vec(tags);
    }
    if let Some(body) = flags.get("body") {
        node.body = body.clone();
    }
    if let Some(imp) = flags.get("importance").and_then(|v| v.parse::<f64>().ok()) {
        node.frontmatter.importance = imp.clamp(0.0, 1.0);
    }
    node.frontmatter.updated = now_iso();

    write_node(&node)?;
    let _ = upsert_index(&node);
    println!("{{\"id\":\"{id}\"}}");
    Ok(0)
}

fn cmd_delete(args: &[String]) -> io::Result<i32> {
    let (pos, _) = parse_flags(args);
    let id = pos
        .first()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "delete requires <id>"))?;
    if !validate_uuid(id) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "invalid node id",
        ));
    }

    delete_node_file(id)?;
    let _ = remove_edges_for_node(id);
    let _ = remove_from_index(id);
    println!("{{\"deleted\":\"{id}\"}}");
    Ok(0)
}

fn cmd_query(args: &[String]) -> io::Result<i32> {
    let (_, flags) = parse_flags(args);
    let limit: usize = flags
        .get("limit")
        .and_then(|l| l.parse().ok())
        .unwrap_or(100);

    let tag = flags.get("tag").map(|s| s.as_str());
    let node_type = flags.get("type").map(|s| s.as_str());
    let project = flags.get("project").map(|s| s.as_str());

    // For agent filter we use query_nodes then post-filter (agents column not indexed)
    let agent = flags.get("agent").cloned();

    let mut nodes = query_nodes(tag, node_type, project, limit);

    if let Some(ref agent_val) = agent {
        nodes.retain(|n| n.frontmatter.agents.contains(agent_val));
    }

    // Convert to IndexNode-like JSON for API compatibility
    let index_nodes: Vec<IndexNode> = nodes
        .iter()
        .map(|n| IndexNode {
            id: n.frontmatter.id.clone(),
            title: n.frontmatter.title.clone(),
            node_type: n.frontmatter.node_type.clone(),
            tags: n.frontmatter.tags.clone(),
            projects: n.frontmatter.projects.clone(),
            updated: n.frontmatter.updated.clone(),
        })
        .collect();

    let out = serde_json::to_string_pretty(&index_nodes)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    println!("{out}");
    Ok(0)
}

fn cmd_search(args: &[String]) -> io::Result<i32> {
    let (pos, flags) = parse_flags(args);
    let query = pos
        .first()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "search requires <query>"))?;
    let limit: usize = flags
        .get("limit")
        .and_then(|l| l.parse().ok())
        .unwrap_or(20);

    let nodes = search_nodes(query, limit);
    let results: Vec<serde_json::Value> = nodes
        .iter()
        .map(|n| {
            serde_json::json!({
                "id":      n.frontmatter.id,
                "title":   n.frontmatter.title,
                "type":    n.frontmatter.node_type,
                "tags":    n.frontmatter.tags,
                "updated": n.frontmatter.updated,
                "snippet": n.body.chars().take(200).collect::<String>()
            })
        })
        .collect();

    let out = serde_json::to_string_pretty(&results)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    println!("{out}");
    Ok(0)
}

fn cmd_related(args: &[String]) -> io::Result<i32> {
    let (pos, flags) = parse_flags(args);
    let id = pos
        .first()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "related requires <id>"))?;
    if !validate_uuid(id) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "invalid node id",
        ));
    }
    let depth: usize = flags.get("depth").and_then(|d| d.parse().ok()).unwrap_or(2);

    let related = related_nodes(id, depth);
    let out = serde_json::to_string_pretty(&related)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    println!("{out}");
    Ok(0)
}

fn cmd_link(args: &[String]) -> io::Result<i32> {
    let (pos, flags) = parse_flags(args);
    if pos.len() < 2 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "link requires <src-id> <dst-id>",
        ));
    }
    let src = &pos[0];
    let dst = &pos[1];
    if !validate_uuid(src) || !validate_uuid(dst) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "invalid node id",
        ));
    }
    let relation = flags
        .get("relation")
        .cloned()
        .unwrap_or_else(|| "related".to_string());
    let weight: f64 = flags
        .get("weight")
        .and_then(|w| w.parse().ok())
        .unwrap_or(1.0);

    let edge = Edge {
        id: Uuid::new_v4().to_string(),
        source: src.clone(),
        target: dst.clone(),
        relation,
        weight,
        ts: now_iso(),
    };

    append_edge(&edge)?;
    println!("{{\"edge_id\":\"{}\"}}", edge.id);
    Ok(0)
}

fn cmd_graph(args: &[String]) -> io::Result<i32> {
    let sub = args.first().map(|s| s.as_str()).unwrap_or("rebuild");
    match sub {
        "rebuild" => {
            rebuild_graph()?;
            println!("{{\"status\":\"ok\"}}");
        }
        _ => {
            eprintln!("Unknown graph subcommand: {sub}");
            return Ok(1);
        }
    }
    Ok(0)
}

fn cmd_validate() -> io::Result<i32> {
    use super::store::list_node_ids;

    // In SQLite mode: nodes stored in DB are always valid.
    // We also check for any legacy .md files in the old nodes dir for completeness.
    let ids = list_node_ids()?;
    let mut errors: Vec<serde_json::Value> = vec![];

    // Check DB nodes can be read back without error (integrity check)
    for id in &ids {
        if super::store::read_node(id).is_err() {
            errors.push(serde_json::json!({
                "id": id,
                "error": "failed to read node from DB"
            }));
        }
    }

    // Also check legacy .md files if they exist in the harness dir
    let legacy_dir = super::store::nodes_dir().join("nodes");
    if legacy_dir.exists() {
        for entry in fs::read_dir(&legacy_dir)? {
            let entry = entry?;
            let path = entry.path();
            let name = path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            if !name.ends_with(".md") {
                continue;
            }
            let content = fs::read_to_string(&path).unwrap_or_default();
            if parse_node(&content).is_none() {
                errors.push(serde_json::json!({
                    "file": name,
                    "error": "failed to parse frontmatter"
                }));
            }
        }
    }

    let out = serde_json::to_string_pretty(&errors)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    println!("{out}");
    Ok(if errors.is_empty() { 0 } else { 1 })
}

fn cmd_migrate(args: &[String]) -> io::Result<i32> {
    let (_, flags) = parse_flags(args);
    let dry_run = flags.contains_key("dry-run");
    let all = flags.contains_key("all");
    let project_filter = flags.get("project").cloned();

    let harness_root = std::env::var("HARNESS_ROOT")
        .or_else(|_| std::env::var("HOME"))
        .unwrap_or_else(|_| "/tmp".to_string());
    let projects_dir = PathBuf::from(&harness_root)
        .join(".harness")
        .join("projects");

    if !projects_dir.exists() {
        println!("{{\"migrated\":0}}");
        return Ok(0);
    }

    let mut migrated = 0;
    let mut results: Vec<serde_json::Value> = vec![];
    let mut all_migrated_nodes: Vec<Node> = vec![];

    let slugs: Vec<String> = if all || project_filter.is_none() {
        fs::read_dir(&projects_dir)?
            .filter_map(|e| e.ok())
            .filter(|e| e.path().is_dir())
            .map(|e| e.file_name().to_string_lossy().to_string())
            .collect()
    } else {
        project_filter.into_iter().collect()
    };

    for slug in &slugs {
        let mem_dir = projects_dir.join(slug).join("memory");
        if !mem_dir.exists() {
            continue;
        }

        for entry in fs::read_dir(&mem_dir)? {
            let entry = entry?;
            let path = entry.path();
            let name = path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            if !name.ends_with(".md") {
                continue;
            }

            let content = fs::read_to_string(&path)?;
            let node = if let Some(n) = parse_node(&content) {
                n
            } else {
                // Auto-generate frontmatter
                let id = Uuid::new_v4().to_string();
                let now = now_iso();
                Node {
                    frontmatter: NodeFrontmatter {
                        id: id.clone(),
                        node_type: "decision".to_string(),
                        title: name.trim_end_matches(".md").to_string(),
                        tags: vec![],
                        projects: vec![slug.clone()],
                        agents: vec![],
                        created: now.clone(),
                        updated: now,
                        importance: importance_for_type("decision"),
                        access_count: 0,
                        accessed_at: String::new(),
                    },
                    body: content.clone(),
                }
            };

            results.push(serde_json::json!({
                "source": path.display().to_string(),
                "id": node.frontmatter.id,
                "dry_run": dry_run
            }));

            if !dry_run {
                write_node(&node)?;
                all_migrated_nodes.push(node);
            }
            migrated += 1;
        }
    }

    // Index is maintained automatically by the SQLite DB (write_node handles it).

    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({
            "migrated": migrated,
            "dry_run": dry_run,
            "nodes": results
        }))
        .unwrap_or_default()
    );
    Ok(0)
}

fn claude_json_path() -> PathBuf {
    crate::hooks::common::claude_json_path()
}

fn cmd_export(args: &[String]) -> io::Result<i32> {
    let (_, flags) = parse_flags(args);
    let dry_run = flags.contains_key("dry-run");

    let out_dir = if let Some(d) = flags.get("out") {
        PathBuf::from(d)
    } else {
        let root = std::env::var("HARNESS_ROOT")
            .or_else(|_| std::env::var("HOME"))
            .unwrap_or_else(|_| "/tmp".to_string());
        PathBuf::from(root).join(".harness").join("exports")
    };

    let ids = list_node_ids()?;
    let mut exported = 0usize;

    if !dry_run {
        fs::create_dir_all(&out_dir)?;
    }

    for id in &ids {
        let Ok(node) = read_node(id) else { continue };
        let filename = format!("{}.md", node.frontmatter.id);
        let path = out_dir.join(&filename);
        let content = serialize_node(&node);

        if dry_run {
            println!("Would write: {}", path.display());
        } else {
            fs::write(&path, &content)?;
        }
        exported += 1;
    }

    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({
            "exported": exported,
            "dry_run":  dry_run,
            "dir":      out_dir.display().to_string(),
        }))
        .unwrap_or_default()
    );
    Ok(0)
}

fn find_epic_harness_binary() -> String {
    "epic-harness".to_string()
}

fn cmd_mcp_install(args: &[String]) -> io::Result<i32> {
    let (_, flags) = parse_flags(args);
    let dry_run = flags.contains_key("dry-run");
    let force = flags.contains_key("force");

    let settings_path = claude_json_path();

    let raw = if settings_path.exists() {
        fs::read_to_string(&settings_path)?
    } else {
        "{}".to_string()
    };

    let mut settings: serde_json::Value = serde_json::from_str(&raw).map_err(|e| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("Failed to parse ~/.claude.json: {e}"),
        )
    })?;

    if settings["mcpServers"]["harness-mem"].is_object() && !force {
        println!("harness-mem already registered (use --force to overwrite)");
        return Ok(0);
    }

    let binary = find_epic_harness_binary();
    let entry = serde_json::json!({
        "command": binary,
        "args": ["mem", "mcp"]
    });

    if dry_run {
        println!(
            "Would add to {}:\n  mcpServers.harness-mem = {}",
            settings_path.display(),
            serde_json::to_string_pretty(&entry).unwrap_or_default()
        );
        return Ok(0);
    }

    settings["mcpServers"]["harness-mem"] = entry;

    let new_content = serde_json::to_string_pretty(&settings)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

    // Atomic write
    if let Some(parent) = settings_path.parent() {
        fs::create_dir_all(parent)?;
    }
    // Use process ID in tmp filename to avoid collisions (file permissions rely on umask, acceptable for local dev tool)
    let tmp_path = settings_path.with_file_name(format!(".claude.{}.json.tmp", std::process::id()));
    fs::write(&tmp_path, &new_content)?;
    fs::rename(&tmp_path, &settings_path)?;

    println!(
        "✓ Registered harness-mem MCP server in {}\n  Restart Claude Code to activate.",
        settings_path.display()
    );
    Ok(0)
}

fn cmd_context(args: &[String]) -> io::Result<i32> {
    let (_, flags) = parse_flags(args);
    let project = flags.get("project").cloned().unwrap_or_default();
    let limit: usize = flags.get("limit").and_then(|l| l.parse().ok()).unwrap_or(5);

    let project_opt = if project.is_empty() {
        None
    } else {
        Some(project.as_str())
    };
    let scored = smart_recall(project_opt, None, limit).unwrap_or_default();

    let results: Vec<serde_json::Value> = scored
        .iter()
        .map(|sn| {
            let fm = &sn.node.frontmatter;
            serde_json::json!({
                "id":           fm.id,
                "title":        fm.title,
                "type":         fm.node_type,
                "tags":         fm.tags,
                "projects":     fm.projects,
                "updated":      fm.updated,
                "importance":   fm.importance,
                "access_count": fm.access_count,
                "score":        (sn.score * 1000.0).round() / 1000.0,
            })
        })
        .collect();

    let out = serde_json::to_string_pretty(&results)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    println!("{out}");
    Ok(0)
}

fn cmd_recall(args: &[String]) -> io::Result<i32> {
    let (pos, flags) = parse_flags(args);
    let hint = pos.first().cloned().unwrap_or_default();
    let project = flags.get("project").cloned();
    let limit: usize = flags
        .get("limit")
        .and_then(|l| l.parse().ok())
        .unwrap_or(10);

    if hint.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "recall requires a hint (describe your current task)",
        ));
    }

    let project_opt = project.as_deref();
    let hint_opt = Some(hint.as_str());
    let scored = smart_recall(project_opt, hint_opt, limit)?;

    let results: Vec<serde_json::Value> = scored
        .iter()
        .map(|sn| {
            let fm = &sn.node.frontmatter;
            serde_json::json!({
                "id":           fm.id,
                "title":        fm.title,
                "type":         fm.node_type,
                "tags":         fm.tags,
                "importance":   fm.importance,
                "access_count": fm.access_count,
                "score":        (sn.score * 1000.0).round() / 1000.0,
                "body":         sn.node.body.chars().take(300).collect::<String>(),
            })
        })
        .collect();

    let out = serde_json::to_string_pretty(&results)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    println!("{out}");
    Ok(0)
}
