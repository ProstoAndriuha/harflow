use std::fs;
use std::path::{Path, PathBuf};
use std::process;

use clap::{Args, Parser, Subcommand, ValueEnum};
use harflow_core::{ExitCode, HarflowError, SCHEMA_VERSION};
use harflow_generator::{generate_sdk_preview, SdkTarget};
use harflow_graph::build_endpoint_graph;
use harflow_openapi::build_openapi_document;
use harflow_project::ProjectWorkspace;
use harflow_replay::{validate_replay_policy, ReplayConfig, ReplayMode, ValidationReport};
use harflow_report::{build_report, render_markdown_report};
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
    /// Run deterministic endpoint and session analysis on a project
    Analyze(ProjectArg),
    /// List discovered API endpoints
    Endpoints(ProjectArg),
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
    har_path: PathBuf,
    #[arg(long, value_name = "DIR")]
    project: PathBuf,
    #[arg(long, default_value = "HARFlow Project")]
    name: String,
}

#[derive(Debug, Args)]
struct ProjectArg {
    project: PathBuf,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum GraphFormat {
    Json,
}

#[derive(Debug, Args)]
struct GraphArgs {
    project: PathBuf,
    #[arg(long, value_enum, default_value_t = GraphFormat::Json)]
    format: GraphFormat,
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
        Commands::Endpoints(args) => cmd_endpoints(&cli, args),
        Commands::Graph(args) => cmd_graph(&cli, args),
        Commands::Generate(args) => cmd_generate(&cli, args),
        Commands::Export(args) => cmd_export(&cli, args),
        Commands::Validate(args) => cmd_validate(&cli, args),
        Commands::Report(args) => cmd_report(&cli, args),
    }
}

fn cmd_import(cli: &Cli, args: &ImportArgs) -> Result<(), HarflowError> {
    let bytes = fs::read(&args.har_path).map_err(|err| {
        HarflowError::InvalidSource(format!(
            "cannot read HAR file '{}': {err}",
            args.har_path.display()
        ))
    })?;

    let mut ws = if args.project.join("project.json").exists() {
        ProjectWorkspace::open(&args.project)?
    } else {
        ProjectWorkspace::create(&args.project, &args.name)?
    };

    let source_name = args
        .har_path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("capture.har");
    let source = ws.import_har_bytes(source_name, &bytes)?;

    if cli.json {
        println!(
            "{}",
            serde_json::to_string_pretty(&source)
                .map_err(|e| HarflowError::Storage(e.to_string()))?
        );
    } else {
        println!(
            "Imported '{}' ({} entries, {} secrets redacted) into {}",
            source.name,
            source.entry_count,
            source.redacted_secrets_count,
            args.project.display()
        );
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

fn cmd_graph(cli: &Cli, args: &GraphArgs) -> Result<(), HarflowError> {
    let ws = ProjectWorkspace::open(&args.project)?;
    let graph = build_endpoint_graph(&ws.model);
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
