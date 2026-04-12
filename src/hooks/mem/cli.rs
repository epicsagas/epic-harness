//! cli.rs — CLI subcommand parsing + dispatch

use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::PathBuf;
use std::process::Command;

use uuid::Uuid;

use super::graph::{rebuild_graph, related_nodes};
use super::store::{
    append_edge, delete_node_file, nodes_dir, now_iso, parse_node, read_index, read_node,
    remove_edges_for_node, remove_from_index, upsert_index, validate_node_id, write_index,
    write_node, Edge, IndexNode, Node, NodeFrontmatter,
};

const SUBCOMMANDS: &[(&str, &str)] = &[
    ("add",         "Add a new memory node"),
    ("edit",        "Edit an existing node"),
    ("delete",      "Delete a node and its edges"),
    ("query",       "List/filter nodes from the index"),
    ("search",      "Full-text search across node files"),
    ("related",     "BFS traversal — find related nodes"),
    ("link",        "Create a directed edge between two nodes"),
    ("graph",       "Manage the graph cache (rebuild)"),
    ("validate",    "Check all node files for parse errors"),
    ("migrate",     "Import legacy project memory files"),
    ("context",     "Show recently-updated nodes for a project"),
    ("mcp-install", "Register the harness-mem MCP server"),
    ("serve",       "Start the REST + Web UI server"),
    ("help",        "Show this help message"),
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
            println!("  --type <type>       Node type: concept|decision|pattern|task|... (default: concept)");
            println!("  --tags <a,b,c>      Comma-separated tags");
            println!("  --project <name>    Associate with a project slug");
            println!("  --agent <name>      Associate with an agent name");
            println!("  --body <text>       Node body content");
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
        "delete" => {
            println!("harness mem delete — Delete a node and its edges\n");
            println!("USAGE:");
            println!("  harness mem delete <ID>\n");
            println!("OUTPUT: {{\"deleted\":\"<uuid>\"}}");
        }
        "query" => {
            println!("harness mem query — List/filter nodes from the index\n");
            println!("USAGE:");
            println!("  harness mem query [OPTIONS]\n");
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
        "mcp-install" => {
            println!("harness mem mcp-install — Register the harness-mem MCP server\n");
            println!("USAGE:");
            println!("  harness mem mcp-install --path <path/to/mem-mcp.cjs> [OPTIONS]\n");
            println!("OPTIONS:");
            println!("  --path <path>       Path to mem-mcp.cjs (required)");
            println!("  --dry-run           Preview without writing settings.json");
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
            return 0;  // help is not an error
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
        "add"         => cmd_add(&args[1..]),
        "edit"        => cmd_edit(&args[1..]),
        "delete"      => cmd_delete(&args[1..]),
        "query"       => cmd_query(&args[1..]),
        "search"      => cmd_search(&args[1..]),
        "related"     => cmd_related(&args[1..]),
        "link"        => cmd_link(&args[1..]),
        "graph"       => cmd_graph(&args[1..]),
        "validate"    => cmd_validate(),
        "migrate"     => cmd_migrate(&args[1..]),
        "context"     => cmd_context(&args[1..]),
        "mcp-install" => cmd_mcp_install(&args[1..]),
        "serve"       => return super::server::serve(&args[1..]),
        "help" | "--help" | "-h" => {
            print_help();
            return 0;
        }
        _ => {
            // "did you mean?" suggestion
            let known: Vec<&str> = SUBCOMMANDS.iter().map(|(n, _)| *n).collect();
            let best = known.iter()
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

    let title = flags.get("title").cloned().unwrap_or_else(|| "Untitled".to_string());
    let node_type = flags.get("type").cloned().unwrap_or_else(|| "concept".to_string());
    let tags = csv_to_vec(flags.get("tags").map(|s| s.as_str()).unwrap_or(""));
    let projects = csv_to_vec(flags.get("project").map(|s| s.as_str()).unwrap_or(""));
    let agents = csv_to_vec(flags.get("agent").map(|s| s.as_str()).unwrap_or(""));
    let body = flags.get("body").cloned().unwrap_or_default();

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
        },
        body,
    };

    write_node(&node)?;
    let _ = upsert_index(&node);

    println!("{{\"id\":\"{id}\"}}");
    Ok(0)
}

fn cmd_edit(args: &[String]) -> io::Result<i32> {
    let (pos, flags) = parse_flags(args);
    let id = pos.first().ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "edit requires <id>"))?;
    if !validate_node_id(id) {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "invalid node id"));
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
    node.frontmatter.updated = now_iso();

    write_node(&node)?;
    let _ = upsert_index(&node);
    println!("{{\"id\":\"{id}\"}}");
    Ok(0)
}

fn cmd_delete(args: &[String]) -> io::Result<i32> {
    let (pos, _) = parse_flags(args);
    let id = pos.first().ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "delete requires <id>"))?;
    if !validate_node_id(id) {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "invalid node id"));
    }

    delete_node_file(id)?;
    let _ = remove_edges_for_node(id);
    let _ = remove_from_index(id);
    println!("{{\"deleted\":\"{id}\"}}");
    Ok(0)
}

fn cmd_query(args: &[String]) -> io::Result<i32> {
    let (_, flags) = parse_flags(args);
    let idx = read_index();

    let mut nodes = idx.nodes.clone();

    if let Some(tag) = flags.get("tag") {
        if let Some(ids) = idx.by_tag.get(tag) {
            let id_set: std::collections::HashSet<_> = ids.iter().collect();
            nodes.retain(|n| id_set.contains(&n.id));
        } else {
            nodes.clear();
        }
    }
    if let Some(t) = flags.get("type") {
        nodes.retain(|n| &n.node_type == t);
    }
    if let Some(proj) = flags.get("project") {
        nodes.retain(|n| n.projects.contains(proj));
    }
    if let Some(agent) = flags.get("agent") {
        // agent filter requires reading each node file
        let filtered: Vec<_> = nodes
            .into_iter()
            .filter(|n| {
                read_node(&n.id)
                    .map(|nd| nd.frontmatter.agents.contains(agent))
                    .unwrap_or(false)
            })
            .collect();
        nodes = filtered;
    }

    let out = serde_json::to_string_pretty(&nodes)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    println!("{out}");
    Ok(0)
}

fn cmd_search(args: &[String]) -> io::Result<i32> {
    let (pos, _) = parse_flags(args);
    let query = pos.first().ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "search requires <query>"))?;
    let dir = nodes_dir();

    // Try rg first, fall back to grep
    let output = Command::new("rg")
        .arg("--line-number")
        .arg("--no-heading")
        .arg("--")
        .arg(query)
        .arg(dir.to_str().unwrap_or("."))
        .output();

    let result = match output {
        Ok(o) if o.status.success() || !o.stdout.is_empty() => {
            String::from_utf8_lossy(&o.stdout).to_string()
        }
        _ => {
            // grep fallback
            let o = Command::new("grep")
                .arg("-rn")
                .arg("--")
                .arg(query)
                .arg(dir.to_str().unwrap_or("."))
                .output()
                .unwrap_or_else(|_| std::process::Output {
                    status: std::process::ExitStatus::default(),
                    stdout: vec![],
                    stderr: vec![],
                });
            String::from_utf8_lossy(&o.stdout).to_string()
        }
    };

    print!("{result}");
    Ok(0)
}

fn cmd_related(args: &[String]) -> io::Result<i32> {
    let (pos, flags) = parse_flags(args);
    let id = pos.first().ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "related requires <id>"))?;
    if !validate_node_id(id) {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "invalid node id"));
    }
    let depth: usize = flags
        .get("depth")
        .and_then(|d| d.parse().ok())
        .unwrap_or(2);

    let related = related_nodes(id, depth);
    let out = serde_json::to_string_pretty(&related)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    println!("{out}");
    Ok(0)
}

fn cmd_link(args: &[String]) -> io::Result<i32> {
    let (pos, flags) = parse_flags(args);
    if pos.len() < 2 {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "link requires <src-id> <dst-id>"));
    }
    let src = &pos[0];
    let dst = &pos[1];
    if !validate_node_id(src) || !validate_node_id(dst) {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "invalid node id"));
    }
    let relation = flags.get("relation").cloned().unwrap_or_else(|| "related".to_string());
    let weight: f64 = flags.get("weight").and_then(|w| w.parse().ok()).unwrap_or(1.0);

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
    let dir = nodes_dir();
    if !dir.exists() {
        println!("[]");
        return Ok(0);
    }

    let mut errors: Vec<serde_json::Value> = vec![];
    for entry in fs::read_dir(&dir)? {
        let entry = entry?;
        let path = entry.path();
        let name = path.file_name().unwrap_or_default().to_string_lossy().to_string();
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
    let projects_dir = PathBuf::from(&harness_root).join(".harness").join("projects");

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
            let name = path.file_name().unwrap_or_default().to_string_lossy().to_string();
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

    // Batch index rebuild: O(N) disk I/O instead of O(N²)
    if !dry_run && !all_migrated_nodes.is_empty() {
        let mut idx = read_index();
        for node in &all_migrated_nodes {
            let fm = &node.frontmatter;
            idx.nodes.retain(|n| n.id != fm.id);
            idx.nodes.push(IndexNode {
                id: fm.id.clone(),
                title: fm.title.clone(),
                node_type: fm.node_type.clone(),
                tags: fm.tags.clone(),
                projects: fm.projects.clone(),
                updated: fm.updated.clone(),
            });
        }
        idx.by_tag.clear();
        idx.by_type.clear();
        idx.by_project.clear();
        for n in &idx.nodes {
            for tag in &n.tags {
                idx.by_tag.entry(tag.clone()).or_default().push(n.id.clone());
            }
            idx.by_type.entry(n.node_type.clone()).or_default().push(n.id.clone());
            for proj in &n.projects {
                idx.by_project.entry(proj.clone()).or_default().push(n.id.clone());
            }
        }
        let _ = write_index(&idx);
    }

    println!("{}", serde_json::to_string_pretty(&serde_json::json!({
        "migrated": migrated,
        "dry_run": dry_run,
        "nodes": results
    })).unwrap_or_default());
    Ok(0)
}

fn claude_settings_path() -> PathBuf {
    if let Ok(p) = std::env::var("CLAUDE_SETTINGS_PATH") {
        return PathBuf::from(p);
    }
    PathBuf::from(std::env::var("HOME").unwrap_or_default())
        .join(".claude")
        .join("settings.json")
}

fn cmd_mcp_install(args: &[String]) -> io::Result<i32> {
    let (_, flags) = parse_flags(args);
    let dry_run = flags.contains_key("dry-run");

    let mcp_path = if let Some(p) = flags.get("path") {
        p.clone()
    } else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "mem-mcp.cjs not found. Specify --path",
        ));
    };

    let settings_path = claude_settings_path();

    let raw = if settings_path.exists() {
        fs::read_to_string(&settings_path)?
    } else {
        "{}".to_string()
    };

    let mut settings: serde_json::Value = serde_json::from_str(&raw)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, format!("Failed to parse settings.json: {e}")))?;

    if settings["mcpServers"]["harness-mem"].is_object() {
        println!("harness-mem already registered");
        return Ok(0);
    }

    let entry = serde_json::json!({
        "command": "node",
        "args": [mcp_path]
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
    let tmp_path = settings_path.with_file_name(format!(
        "settings.{}.json.tmp",
        std::process::id()
    ));
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

    let idx = read_index();
    let mut nodes = idx.nodes.clone();

    if !project.is_empty() {
        nodes.retain(|n| n.projects.contains(&project));
    }

    // Sort by updated descending
    nodes.sort_by(|a, b| b.updated.cmp(&a.updated));
    nodes.truncate(limit);

    let out = serde_json::to_string_pretty(&nodes)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    println!("{out}");
    Ok(0)
}
