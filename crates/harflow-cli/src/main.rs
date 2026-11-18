use std::fs::{self, File};
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::process;

use clap::{Args, Parser, Subcommand, ValueEnum};
use harflow_core::{ExitCode, HarflowError, SCHEMA_VERSION};
use harflow_generator::{generate_sdk_preview, SdkTarget};
use harflow_graph::GraphKind;
use harflow_har::ImportLimits;
use harflow_model::ConfirmationStatus;
use harflow_openapi::build_openapi_document;
use harflow_project::ProjectWorkspace;
use harflow_replay::{validate_replay_policy, ReplayConfig, ReplayMode, ValidationReport};
use harflow_report::{build_report, render_markdown_report};
use serde::Deserialize;
use serde_json::json;

#[derive(Debug, Parser)]
#[command(
    name = "harflow",
    version,
    about = "Local-first HTTP traffic analyzer and async SDK generator"
)]
struct Cli {
    #[arg(long, global = true, help = "Emit machine-readable JSON output")]
    json: bool,

    #[arg(long, global = true, value_name = "DIR", help = "Output directory")]
    output: Option<PathBuf>,

    #[arg(
        long,
        global = true,
        value_name = "FILE",
        help = "Optional config file"
    )]
    config: Option<PathBuf>,

    #[arg(
        long,
        global = true,
        help = "Treat critical warnings or unknown mappings as errors"
    )]
    strict: bool,

    #[arg(
        long,
        global = true,
        default_value_t = true,
        help = "Disable all outbound network requests"
    )]
    offline: bool,

    #[arg(long, global = true, help = "Disable ANSI color formatting")]
    no_color: bool,

    #[arg(long, global = true, help = "Include additional safe diagnostics")]
    verbose: bool,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Import a HAR 1.2 file into a local HARFlow project (redacts secrets automatically)
    Import(ImportArgs),
    /// Run deterministic endpoint, cleaner, schema, and session analysis on a project
    Analyze(ProjectArg),
    /// Preview or update Smart Traffic Cleaner include/exclude rules
    Clean(CleanArgs),
    /// List discovered API endpoints
    Endpoints(ProjectArg),
    /// List inferred request and response JSON schemas
    Schemas(ProjectArg),
    /// Inspect, confirm, or reject detected API dependencies
    Dependencies(DependenciesArgs),
    /// Inspect authentication mechanisms, token rotations, or Multi-HAR session diffs
    Sessions(SessionsArgs),
    /// Build and print the project dependency/endpoint graph
    Graph(GraphArgs),
    /// Generate a typed async SDK preview (python or typescript)
    Generate(GenerateArgs),
    /// Export specifications such as OpenAPI 3.1
    Export(ExportArgs),
    /// Run mock or explicitly authorized live validation
    Validate(ValidateArgs),
    /// Produce a structured Markdown or JSON analysis report
    Report(ReportArgs),
    /// Verify local CLI health, schema version, and offline security posture
    Doctor,
}

#[derive(Debug, Args)]
struct ImportArgs {
    /// One or more HAR 1.2 or .har.gz files to stream into the project
    #[arg(required = true, num_args = 1..)]
    har_paths: Vec<PathBuf>,
    #[arg(long, value_name = "DIR")]
    project: PathBuf,
    #[arg(long, default_value = "HARFlow Project")]
    name: String,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CliConfig {
    max_har_size_bytes: Option<usize>,
    max_entries: Option<usize>,
    max_json_depth: Option<usize>,
    tolerate_partial_entries: Option<bool>,
}

#[derive(Debug, Args)]
struct ProjectArg {
    project: PathBuf,
}

#[derive(Debug, Args)]
struct CleanArgs {
    project: PathBuf,
    #[arg(long, help = "Preview classification without modifying settings")]
    preview: bool,
    #[arg(long = "exclude-domain", value_name = "DOMAIN")]
    exclude_domains: Vec<String>,
    #[arg(long = "include-domain", value_name = "DOMAIN")]
    include_domains: Vec<String>,
    #[arg(long = "exclude-request", value_name = "REQUEST_ID")]
    exclude_requests: Vec<String>,
    #[arg(long = "restore-request", value_name = "REQUEST_ID")]
    restore_requests: Vec<String>,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum CliConfirmationStatus {
    Confirmed,
    Likely,
    Unknown,
    Rejected,
    #[value(name = "user-confirmed", alias = "user_confirmed")]
    UserConfirmed,
}

impl From<CliConfirmationStatus> for ConfirmationStatus {
    fn from(value: CliConfirmationStatus) -> Self {
        match value {
            CliConfirmationStatus::Confirmed => Self::Confirmed,
            CliConfirmationStatus::Likely => Self::Likely,
            CliConfirmationStatus::Unknown => Self::Unknown,
            CliConfirmationStatus::Rejected => Self::Rejected,
            CliConfirmationStatus::UserConfirmed => Self::UserConfirmed,
        }
    }
}

#[derive(Debug, Args)]
struct DependenciesArgs {
    project: PathBuf,
    #[arg(long, value_enum, help = "Filter dependencies by status")]
    status: Option<CliConfirmationStatus>,
    #[arg(long = "confirm", value_name = "DEPENDENCY_ID")]
    confirm_ids: Vec<String>,
    #[arg(long = "reject", value_name = "DEPENDENCY_ID")]
    reject_ids: Vec<String>,
}

#[derive(Debug, Args)]
struct SessionsArgs {
    project: PathBuf,
    #[arg(long, help = "Compare multiple imported HAR sessions")]
    compare: bool,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum GraphFormat {
    Json,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum CliGraphKind {
    Endpoint,
    #[value(name = "request-dependency", alias = "request_dependency")]
    RequestDependency,
    #[value(name = "auth-flow", alias = "auth_flow")]
    AuthFlow,
    #[value(name = "session-timeline", alias = "session_timeline")]
    SessionTimeline,
    #[value(name = "data-flow", alias = "data_flow")]
    DataFlow,
}

impl From<CliGraphKind> for GraphKind {
    fn from(value: CliGraphKind) -> Self {
        match value {
            CliGraphKind::Endpoint => Self::Endpoint,
            CliGraphKind::RequestDependency => Self::RequestDependency,
            CliGraphKind::AuthFlow => Self::AuthFlow,
            CliGraphKind::SessionTimeline => Self::SessionTimeline,
            CliGraphKind::DataFlow => Self::DataFlow,
        }
    }
}

#[derive(Debug, Args)]
struct GraphArgs {
    project: PathBuf,
    #[arg(long, value_enum, default_value_t = GraphFormat::Json)]
    format: GraphFormat,
    #[arg(long, value_enum, default_value_t = CliGraphKind::Endpoint)]
    kind: CliGraphKind,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum SdkLang {
    Python,
    Typescript,
}

#[derive(Debug, Args)]
struct GenerateArgs {
    project: PathBuf,
    #[arg(long, value_enum, default_value_t = SdkLang::Python)]
    lang: SdkLang,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum ExportFormat {
    Openapi,
}

#[derive(Debug, Args)]
struct ExportArgs {
    project: PathBuf,
    #[arg(long, value_enum, default_value_t = ExportFormat::Openapi)]
    format: ExportFormat,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum CliReplayMode {
    Mock,
    Live,
}

#[derive(Debug, Args)]
struct ValidateArgs {
    project: PathBuf,
    #[arg(long, value_enum, default_value_t = CliReplayMode::Mock)]
    mode: CliReplayMode,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum ReportFormat {
    Markdown,
    Json,
}

#[derive(Debug, Args)]
struct ReportArgs {
    project: PathBuf,
    #[arg(long, value_enum, default_value_t = ReportFormat::Markdown)]
    format: ReportFormat,
}

fn main() {
    let cli = match Cli::try_parse() {
        Ok(parsed) => parsed,
        Err(err) => {
            let kind = err.kind();
            let is_info = matches!(
                kind,
                clap::error::ErrorKind::DisplayHelp | clap::error::ErrorKind::DisplayVersion
            );
            let _ = err.print();
            let code = if is_info {
                ExitCode::Success
            } else {
                ExitCode::InvalidArgs
            };
            process::exit(code.as_i32());
        }
    };

    match run(cli) {
        Ok(()) => process::exit(ExitCode::Success.as_i32()),
        Err(err) => {
            eprintln!("error: {err}");
            process::exit(err.exit_code().as_i32());
        }
    }
}

fn run(cli: Cli) -> Result<(), HarflowError> {
    match &cli.command {
        Commands::Doctor => {
            let payload = json!({
                "status": "ok",
                "cliVersion": env!("CARGO_PKG_VERSION"),
                "schemaVersion": SCHEMA_VERSION,
                "offlineDefault": cli.offline,
                "configLoaded": cli.config.as_ref().is_some_and(|p| p.exists()),
                "noColor": cli.no_color,
                "verbose": cli.verbose,
            });
            if cli.json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&payload)
                        .map_err(|e| HarflowError::Storage(e.to_string()))?
                );
            } else {
                println!(
                    "harflow {} (schema {}, offline={})",
                    env!("CARGO_PKG_VERSION"),
                    SCHEMA_VERSION,
                    cli.offline
                );
            }
            Ok(())
        }
        Commands::Import(args) => cmd_import(&cli, args),
        Commands::Analyze(args) => cmd_analyze(&cli, args),
        Commands::Clean(args) => cmd_clean(&cli, args),
        Commands::Endpoints(args) => cmd_endpoints(&cli, args),
        Commands::Schemas(args) => cmd_schemas(&cli, args),
        Commands::Dependencies(args) => cmd_dependencies(&cli, args),
        Commands::Sessions(args) => cmd_sessions(&cli, args),
        Commands::Graph(args) => cmd_graph(&cli, args),
        Commands::Generate(args) => cmd_generate(&cli, args),
        Commands::Export(args) => cmd_export(&cli, args),
        Commands::Validate(args) => cmd_validate(&cli, args),
        Commands::Report(args) => cmd_report(&cli, args),
    }
}

fn load_import_limits(
    config_path: Option<&Path>,
    project_default_bytes: usize,
) -> Result<ImportLimits, HarflowError> {
    let mut limits = ImportLimits {
        default_max_bytes: project_default_bytes,
        ..ImportLimits::default()
    };

    let Some(path) = config_path else {
        return Ok(limits);
    };

    let raw = fs::read_to_string(path).map_err(|err| {
        HarflowError::InvalidArgs(format!(
            "cannot read config file '{}': {err}",
            path.display()
        ))
    })?;
    let cfg: CliConfig = serde_json::from_str(&raw).map_err(|err| {
        HarflowError::InvalidArgs(format!("invalid config JSON '{}': {err}", path.display()))
    })?;

    if let Some(max_bytes) = cfg.max_har_size_bytes {
        if max_bytes == 0 || max_bytes > limits.hard_max_bytes {
            return Err(HarflowError::InvalidArgs(format!(
                "maxHarSizeBytes ({max_bytes}) must be within 1..={}",
                limits.hard_max_bytes
            )));
        }
        limits.default_max_bytes = max_bytes;
    }
    if let Some(max_entries) = cfg.max_entries {
        if max_entries == 0 {
            return Err(HarflowError::InvalidArgs(
                "maxEntries must be greater than zero".to_string(),
            ));
        }
        limits.max_entries = max_entries;
    }
    if let Some(max_depth) = cfg.max_json_depth {
        if max_depth == 0 {
            return Err(HarflowError::InvalidArgs(
                "maxJsonDepth must be greater than zero".to_string(),
            ));
        }
        limits.max_json_depth = max_depth;
    }
    if let Some(tolerate) = cfg.tolerate_partial_entries {
        limits.tolerate_partial_entries = tolerate;
    }

    Ok(limits)
}

fn cmd_import(cli: &Cli, args: &ImportArgs) -> Result<(), HarflowError> {
    let mut ws = if args.project.join("project.json").exists() {
        ProjectWorkspace::open(&args.project)?
    } else {
        ProjectWorkspace::create(&args.project, &args.name)?
    };

    let limits = load_import_limits(
        cli.config.as_deref(),
        ws.manifest.settings.max_har_size_bytes,
    )?;

    let mut imported_sources = Vec::with_capacity(args.har_paths.len());
    let mut skipped_total = 0usize;

    for har_path in &args.har_paths {
        let file = File::open(har_path).map_err(|err| {
            HarflowError::InvalidSource(format!(
                "cannot open HAR file '{}': {err}",
                har_path.display()
            ))
        })?;
        let reader = BufReader::with_capacity(64 * 1024, file);
        let source_name = har_path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("capture.har");

        let (source, summary) = ws.import_har_reader(source_name, reader, Some(limits))?;
        if cli.strict && !summary.skipped_entries.is_empty() {
            return Err(HarflowError::InvalidSource(format!(
                "strict mode failed: {} malformed/partial entries skipped in '{source_name}'",
                summary.skipped_entries.len()
            )));
        }
        skipped_total += summary.skipped_entries.len();
        imported_sources.push((source, summary.skipped_entries.len()));
    }

    if cli.json {
        let rendered = if imported_sources.len() == 1 {
            let (ref src, skipped) = imported_sources[0];
            json!({
                "id": src.id,
                "name": src.name,
                "harVersion": src.har_version,
                "importedAt": src.imported_at,
                "entryCount": src.entry_count,
                "redactedSecretsCount": src.redacted_secrets_count,
                "skippedEntriesCount": skipped,
            })
        } else {
            json!({
                "importedCount": imported_sources.len(),
                "skippedEntriesTotal": skipped_total,
                "sources": imported_sources.iter().map(|(s, skipped)| json!({
                    "id": s.id,
                    "name": s.name,
                    "entryCount": s.entry_count,
                    "redactedSecretsCount": s.redacted_secrets_count,
                    "skippedEntriesCount": skipped,
                })).collect::<Vec<_>>(),
            })
        };
        println!(
            "{}",
            serde_json::to_string_pretty(&rendered)
                .map_err(|e| HarflowError::Storage(e.to_string()))?
        );
    } else {
        for (src, skipped) in &imported_sources {
            println!(
                "Imported '{}' ({} entries, {} secrets redacted, {} skipped) into {}",
                src.name,
                src.entry_count,
                src.redacted_secrets_count,
                skipped,
                args.project.display()
            );
        }
    }
    Ok(())
}

fn cmd_analyze(cli: &Cli, args: &ProjectArg) -> Result<(), HarflowError> {
    let mut ws = ProjectWorkspace::open(&args.project)?;
    ws.analyze()?;

    if cli.strict && ws.manifest.statistics.unknown_mappings > 0 {
        return Err(HarflowError::Analysis(format!(
            "strict mode failed: {} unknown dependency mappings remain",
            ws.manifest.statistics.unknown_mappings
        )));
    }

    if cli.json {
        println!(
            "{}",
            serde_json::to_string_pretty(&ws.manifest.statistics)
                .map_err(|e| HarflowError::Storage(e.to_string()))?
        );
    } else {
        println!(
            "Analyzed {}: {} requests, {} endpoints, {} auth flows",
            ws.manifest.name,
            ws.manifest.statistics.total_requests,
            ws.manifest.statistics.unique_endpoints,
            ws.manifest.statistics.auth_flows
        );
    }
    Ok(())
}

fn cmd_clean(cli: &Cli, args: &CleanArgs) -> Result<(), HarflowError> {
    let mut ws = ProjectWorkspace::open(&args.project)?;
    let has_mutations = !args.exclude_domains.is_empty()
        || !args.include_domains.is_empty()
        || !args.exclude_requests.is_empty()
        || !args.restore_requests.is_empty();

    if !args.preview && has_mutations {
        for domain in &args.exclude_domains {
            ws.exclude_domain(domain)?;
        }
        for domain in &args.include_domains {
            ws.include_domain(domain)?;
        }
        for req_id in &args.exclude_requests {
            ws.exclude_request(req_id)?;
        }
        for req_id in &args.restore_requests {
            ws.restore_request(req_id)?;
        }
    }

    let preview = ws.preview_cleaning();
    if cli.json {
        println!(
            "{}",
            serde_json::to_string_pretty(&preview)
                .map_err(|e| HarflowError::Storage(e.to_string()))?
        );
    } else {
        println!(
            "Cleaner summary: {} total, {} kept, {} excluded",
            preview.total_entries, preview.kept_entries, preview.excluded_entries
        );
    }
    Ok(())
}

fn cmd_endpoints(cli: &Cli, args: &ProjectArg) -> Result<(), HarflowError> {
    let ws = ProjectWorkspace::open(&args.project)?;
    if cli.json {
        println!(
            "{}",
            serde_json::to_string_pretty(&ws.model.endpoints)
                .map_err(|e| HarflowError::Storage(e.to_string()))?
        );
    } else {
        for ep in &ws.model.endpoints {
            println!("{} {} ({})", ep.method, ep.path_template, ep.category);
        }
    }
    Ok(())
}

fn cmd_schemas(cli: &Cli, args: &ProjectArg) -> Result<(), HarflowError> {
    let ws = ProjectWorkspace::open(&args.project)?;
    if cli.json {
        println!(
            "{}",
            serde_json::to_string_pretty(&ws.model.schemas)
                .map_err(|e| HarflowError::Storage(e.to_string()))?
        );
    } else {
        for schema in &ws.model.schemas {
            println!(
                "{} ({}, observations={}, fields={})",
                schema.name,
                schema.root_type,
                schema.observation_count,
                schema.fields.len()
            );
        }
    }
    Ok(())
}

fn cmd_dependencies(cli: &Cli, args: &DependenciesArgs) -> Result<(), HarflowError> {
    let mut ws = ProjectWorkspace::open(&args.project)?;
    for id in &args.confirm_ids {
        ws.set_dependency_status(id, ConfirmationStatus::UserConfirmed)?;
    }
    for id in &args.reject_ids {
        ws.set_dependency_status(id, ConfirmationStatus::Rejected)?;
    }

    let filter = args.status.map(ConfirmationStatus::from);
    let filtered: Vec<_> = ws
        .model
        .dependencies
        .iter()
        .filter(|dep| filter.is_none_or(|expected| dep.status == expected))
        .cloned()
        .collect();

    if cli.json {
        println!(
            "{}",
            serde_json::to_string_pretty(&filtered)
                .map_err(|e| HarflowError::Storage(e.to_string()))?
        );
    } else if filtered.is_empty() {
        println!("No matching dependencies");
    } else {
        for dep in &filtered {
            println!(
                "{} [{:?}] {:.2} {}:{} -> {}:{} ({:?})",
                dep.id,
                dep.status,
                dep.confidence,
                dep.source.request_id,
                dep.source.json_path_or_key,
                dep.target.request_id,
                dep.target.json_path_or_key,
                dep.dependency_type
            );
        }
    }
    Ok(())
}

fn cmd_sessions(cli: &Cli, args: &SessionsArgs) -> Result<(), HarflowError> {
    let ws = ProjectWorkspace::open(&args.project)?;
    if args.compare {
        let comparison = ws.compare_sessions();
        if cli.json {
            println!(
                "{}",
                serde_json::to_string_pretty(&comparison)
                    .map_err(|e| HarflowError::Storage(e.to_string()))?
            );
        } else {
            println!(
                "Session comparison ({} sessions): {} shared endpoints, {} session-only endpoints, {} corroborated deps, {} schema drifts",
                comparison.session_ids.len(),
                comparison.shared_endpoints.len(),
                comparison.session_only_endpoints.len(),
                comparison.corroborated_dependency_ids.len(),
                comparison.schema_drifts.len()
            );
        }
    } else {
        let lifecycle = ws.session_lifecycle();
        if cli.json {
            println!(
                "{}",
                serde_json::to_string_pretty(&lifecycle)
                    .map_err(|e| HarflowError::Storage(e.to_string()))?
            );
        } else {
            println!(
                "Session lifecycle: {} auth flows, {} rotations, {} expiration signals, autoRefreshEligible={}",
                lifecycle.auth_flows.len(),
                lifecycle.token_rotations.len(),
                lifecycle.expiration_signals.len(),
                lifecycle.auto_refresh_eligible
            );
        }
    }
    Ok(())
}

fn cmd_graph(cli: &Cli, args: &GraphArgs) -> Result<(), HarflowError> {
    let ws = ProjectWorkspace::open(&args.project)?;
    let graph = ws.build_graph(args.kind.into());
    let _ = args.format;
    let rendered =
        serde_json::to_string_pretty(&graph).map_err(|e| HarflowError::Storage(e.to_string()))?;

    if let Some(out_dir) = &cli.output {
        write_output_file(out_dir, "graph.json", &rendered)?;
    }
    println!("{rendered}");
    Ok(())
}

fn cmd_generate(cli: &Cli, args: &GenerateArgs) -> Result<(), HarflowError> {
    let ws = ProjectWorkspace::open(&args.project)?;
    let target = match args.lang {
        SdkLang::Python => SdkTarget::PythonHttpx,
        SdkLang::Typescript => SdkTarget::TypeScriptFetch,
    };
    let bundle = generate_sdk_preview(&ws.model, target)?;

    if let Some(out_dir) = &cli.output {
        for file in &bundle.files {
            write_output_file(out_dir, &file.path, &file.content)?;
        }
    }

    if cli.json {
        println!(
            "{}",
            serde_json::to_string_pretty(&bundle)
                .map_err(|e| HarflowError::Storage(e.to_string()))?
        );
    } else {
        println!("Generated {} SDK files", bundle.files.len());
    }
    Ok(())
}

fn cmd_export(cli: &Cli, args: &ExportArgs) -> Result<(), HarflowError> {
    let ws = ProjectWorkspace::open(&args.project)?;
    let _ = args.format;
    let doc = build_openapi_document(&ws.model);
    let rendered =
        serde_json::to_string_pretty(&doc).map_err(|e| HarflowError::Storage(e.to_string()))?;

    if let Some(out_dir) = &cli.output {
        write_output_file(out_dir, "openapi.json", &rendered)?;
    }
    println!("{rendered}");
    Ok(())
}

fn cmd_validate(cli: &Cli, args: &ValidateArgs) -> Result<(), HarflowError> {
    let ws = ProjectWorkspace::open(&args.project)?;
    let cfg = ReplayConfig {
        mode: match args.mode {
            CliReplayMode::Mock => ReplayMode::Mock,
            CliReplayMode::Live => ReplayMode::AuthorizedLive,
        },
        offline_only: cli.offline,
        ..ReplayConfig::default()
    };

    for ep in &ws.model.endpoints {
        validate_replay_policy(&cfg, &ep.method, &ep.host)?;
    }

    let report = ValidationReport {
        total_checks: ws.model.endpoints.len(),
        passed: ws.model.endpoints.len(),
        failed: 0,
        skipped: 0,
    };

    if cli.json {
        println!(
            "{}",
            serde_json::to_string_pretty(&report)
                .map_err(|e| HarflowError::Storage(e.to_string()))?
        );
    } else {
        println!(
            "Validation passed ({}/{})",
            report.passed, report.total_checks
        );
    }
    Ok(())
}

fn cmd_report(cli: &Cli, args: &ReportArgs) -> Result<(), HarflowError> {
    let ws = ProjectWorkspace::open(&args.project)?;
    let report = build_report(&ws.model);

    let use_json = cli.json || matches!(args.format, ReportFormat::Json);
    let (filename, content) = if use_json {
        (
            "analysis-report.json",
            serde_json::to_string_pretty(&report)
                .map_err(|e| HarflowError::Storage(e.to_string()))?,
        )
    } else {
        ("analysis-report.md", render_markdown_report(&report))
    };

    if let Some(out_dir) = &cli.output {
        write_output_file(out_dir, filename, &content)?;
    }
    println!("{content}");
    Ok(())
}

fn write_output_file(
    out_dir: &Path,
    relative_path: &str,
    content: &str,
) -> Result<(), HarflowError> {
    let target = out_dir.join(relative_path);
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(target, content)?;
    Ok(())
}
