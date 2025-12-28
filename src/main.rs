use anyhow::{anyhow, Context, Result};
use clap::{Parser, Subcommand};
use crossterm::{
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, is_raw_mode_enabled, Clear, ClearType},
    event::{self, Event, KeyCode},
    cursor::{Hide, Show, MoveTo, EnableBlinking, DisableBlinking},
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs;
use std::io::{self, Write};
use std::path::PathBuf;
use std::process::Command;

const PROVIDERS_DIR: &str = "providers";
const SETTINGS_FILE: &str = "settings.json";

#[derive(Debug, Clone, Copy, PartialEq)]
enum Shell {
    Bash,
    Zsh,
}

fn detect_shell() -> Shell {
    std::env::var("SHELL")
        .ok()
        .and_then(|s| {
            if s.contains("zsh") {
                Some(Shell::Zsh)
            } else if s.contains("bash") {
                Some(Shell::Bash)
            } else {
                None
            }
        })
        .unwrap_or_else(|| {
            // Fallback: check TERM to help determine shell
            match std::env::var("TERM").as_deref() {
                Ok(_) => Shell::Zsh, // Default to zsh as fallback when in terminal
                Err(_) => Shell::Zsh,
            }
        })
}

impl Shell {
    fn func_file_name(&self) -> &str {
        match self {
            Shell::Bash => "provider-functions.bash",
            Shell::Zsh => "provider-functions.zsh",
        }
    }

    fn rc_file_name(&self) -> &str {
        match self {
            Shell::Bash => ".bashrc",
            Shell::Zsh => ".zshrc",
        }
    }

    fn source_command(&self, path: &std::path::Path) -> String {
        match self {
            Shell::Bash => format!("source {}", path.display()),
            Shell::Zsh => format!("source {}", path.display()),
        }
    }
}

#[derive(Serialize, Deserialize, Default, Debug)]
struct EnvSettings {
    #[serde(alias = "anthropic_base_url")]
    anthropic_base_url: Option<String>,
    #[serde(alias = "anthropic_auth_token")]
    anthropic_auth_token: Option<String>,
    #[serde(alias = "api_timeout_ms")]
    api_timeout_ms: Option<String>,
    #[serde(alias = "claude_code_disable_nonessential_traffic")]
    claude_code_disable_nonessential_traffic: Option<i32>,
    #[serde(alias = "anthropic_model")]
    anthropic_model: Option<String>,
    #[serde(alias = "anthropic_small_fast_model")]
    anthropic_small_fast_model: Option<String>,
    #[serde(alias = "anthropic_default_sonnet_model")]
    anthropic_default_sonnet_model: Option<String>,
    #[serde(alias = "anthropic_default_opus_model")]
    anthropic_default_opus_model: Option<String>,
    #[serde(alias = "anthropic_default_haiku_model")]
    anthropic_default_haiku_model: Option<String>,
}

#[derive(Serialize, Deserialize, Debug)]
struct ClaudeSettings {
    #[serde(default)]
    env: EnvSettings,
    #[serde(default)]
    enabled_plugins: Value,
    #[serde(default)]
    always_thinking_enabled: Option<bool>,
    #[serde(flatten)]
    other: Value,
}

fn get_config_dir() -> Result<PathBuf> {
    let claude_dir = PathBuf::from(env!("HOME")).join(".claude");
    if !claude_dir.exists() {
        fs::create_dir_all(&claude_dir)?;
    }
    Ok(claude_dir)
}

fn get_providers_dir() -> PathBuf {
    get_config_dir().unwrap_or_else(|_| {
        PathBuf::from(env!("HOME")).join(".claude").join(PROVIDERS_DIR)
    })
}

fn ensure_providers_dir() -> Result<PathBuf> {
    let dir = get_providers_dir();
    if !dir.exists() {
        fs::create_dir_all(&dir)?;
    }
    Ok(dir)
}

fn clear_screen() {
    let mut stdout = io::stdout();
    
    let _ = execute!(stdout, Clear(ClearType::All));
    let _ = execute!(stdout, MoveTo(0, 0));
    stdout.flush().unwrap();
}

struct RawModeGuard;

impl RawModeGuard {
    fn new() -> Result<Self> {
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        let _ = execute!(stdout, Hide);
        let _ = execute!(stdout, EnableBlinking);
        Ok(Self)
    }
}

impl Drop for RawModeGuard {
    fn drop(&mut self) {
        let mut stdout = io::stdout();
        let _ = execute!(stdout, Show);
        let _ = execute!(stdout, DisableBlinking);
        if is_raw_mode_enabled().unwrap_or(false) {
            disable_raw_mode().ok();
        }
        stdout.flush().unwrap();
    }
}

fn prompt_input(prompt: &str) -> Result<String> {
    print!("{}", prompt);
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    Ok(input.trim().to_string())
}

fn wait_for_key() {
    let mut stdout = io::stdout();
    let _ = execute!(stdout, Show);
    let _ = execute!(stdout, DisableBlinking);
    print!("  Press Enter to continue...");
    io::stdout().flush().unwrap();

    
    let mut input = String::new();
    let _ = io::stdin().read_line(&mut input);

    let _ = execute!(stdout, Hide);
}

fn draw_menu_with_arrows(options: &[&str], title: &str) -> usize {
    let mut selected = 0;

    loop {
        clear_screen();

        
        let mut stdout = io::stdout();
        execute!(stdout, MoveTo(0, 0)).unwrap();
        let lines = [
            "╔══════════════════════════════════════════════════════════╗",
            &format!("║ {:^56} ║", title),
            "╚══════════════════════════════════════════════════════════╝",
            "",
        ];
        for (i, line) in lines.iter().enumerate() {
            execute!(stdout, MoveTo(0, i as u16)).unwrap();
            execute!(stdout, Clear(ClearType::UntilNewLine)).unwrap();
            println!("{}", line);
        }
        stdout.flush().unwrap();

        
        for (i, opt) in options.iter().enumerate() {
            let marker = if i == selected { ">" } else { " " };
            let y = i as u16 + 6;
            execute!(stdout, MoveTo(0, y)).unwrap();
            execute!(stdout, Clear(ClearType::UntilNewLine)).unwrap();
            println!("  {}  {}", marker, opt);
        }

        
        let y = (options.len() as u16) + 8;
        execute!(stdout, MoveTo(0, y)).unwrap();
        execute!(stdout, Clear(ClearType::UntilNewLine)).unwrap();
        print!("  Use ↑/↓ arrows to navigate, Enter to select, Esc to go back");
        stdout.flush().unwrap();

        if let Ok(Event::Key(key)) = event::read() {
            match key.code {
                KeyCode::Up | KeyCode::Char('k') => {
                    if selected > 0 {
                        selected -= 1;
                    }
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    if selected < options.len() - 1 {
                        selected += 1;
                    }
                }
                KeyCode::Enter => {
                    return selected;
                }
                KeyCode::Esc => {
                    return options.len(); 
                }
                _ => {}
            }
        }
    }
}

fn prompt_password(prompt: &str) -> Result<String> {
    prompt_input(prompt)
}

fn remove_provider_function_from_file(func_path: &PathBuf, provider_name: &str) -> Result<()> {
    if !func_path.exists() {
        return Ok(());
    }

    let existing_funcs = fs::read_to_string(func_path)?;
    let marker = format!("# Provider function for {}", provider_name);
    let mut cleaned_lines = Vec::new();
    let mut in_function_block = false;

    for line in existing_funcs.lines() {
        if line.starts_with(&marker) {
            in_function_block = true;
            continue;
        }
        if in_function_block {
            if line.trim() == "}" {
                in_function_block = false;
            }
            continue;
        }
        cleaned_lines.push(line);
    }

    let cleaned = cleaned_lines.join("\n").trim().to_string();
    if cleaned.is_empty() {
        fs::remove_file(func_path)?;
    } else {
        fs::write(func_path, cleaned)?;
    }
    Ok(())
}

fn remove_provider_function(provider_name: &str) -> Result<()> {
    let config_dir = get_config_dir()?;
    let shells = [Shell::Bash, Shell::Zsh];

    for shell in &shells {
        let func_path = config_dir.join(shell.func_file_name());
        remove_provider_function_from_file(&func_path, provider_name)?;
    }

    Ok(())
}

fn append_provider_function_to_file(func_path: &PathBuf, rc_path: &PathBuf, name: &str, shell: Shell) -> Result<()> {
    let func_content = format!(
        r#"# Provider function for {name}
{name}() {{
    claude-provider use {name} "$@"
}}
"#,
        name = name
    );

    let existing = if func_path.exists() {
        fs::read_to_string(func_path)?
    } else {
        String::new()
    };

    let marker = format!("# Provider function for {}", name);
    let mut cleaned_lines = Vec::new();
    let mut in_function_block = false;
    for line in existing.lines() {
        if line.starts_with(&marker) {
            in_function_block = true;
            continue;
        }
        if in_function_block {
            if line.trim() == "}" {
                in_function_block = false;
            }
            continue;
        }
        cleaned_lines.push(line);
    }

    let cleaned = cleaned_lines.join("\n").trim().to_string();
    let new_content = if cleaned.is_empty() {
        func_content
    } else {
        format!("{}\n\n{}", cleaned, func_content)
    };

    fs::write(func_path, new_content)?;

    let source_line = shell.source_command(func_path);

    if rc_path.exists() {
        let rc_content = fs::read_to_string(rc_path)?;
        if !rc_content.contains(&source_line) {
            fs::write(rc_path, format!("{}\n{}\n", rc_content.trim(), source_line))?;
            println!("  ✓ Added source line to ~/{}", shell.rc_file_name());
        }
    } else {
        fs::write(rc_path, format!("{}\n", source_line))?;
        println!("  ✓ Created ~/{} with source line", shell.rc_file_name());
    }

    Ok(())
}

fn append_provider_function(name: &str) -> Result<()> {
    let config_dir = get_config_dir()?;
    let home = PathBuf::from(env!("HOME"));

    for shell in [Shell::Bash, Shell::Zsh] {
        let func_path = config_dir.join(shell.func_file_name());
        let rc_path = home.join(shell.rc_file_name());
        append_provider_function_to_file(&func_path, &rc_path, name, shell)?;
    }

    Ok(())
}

fn list_providers() -> Result<Vec<String>> {
    let dir = ensure_providers_dir()?;
    let mut providers = Vec::new();

    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        if entry.path().extension().and_then(|e| e.to_str()) == Some("json") {
            if let Some(name) = entry.path().file_stem().and_then(|n| n.to_str()) {
                providers.push(name.to_string());
            }
        }
    }

    providers.sort();
    Ok(providers)
}

fn setup_provider_interactive() -> Result<()> {
    println!();
    println!("  ▸ Configure a new Claude Code provider");
    println!();

    let name = prompt_input("  Enter provider name (e.g., minimax, zai): ")?;
    if name.is_empty() {
        return Err(anyhow!("Provider name cannot be empty"));
    }

    let base_url = prompt_input("  Enter API base URL: ")?;
    if base_url.is_empty() {
        return Err(anyhow!("Base URL cannot be empty"));
    }

    let api_key = prompt_password("  Enter API key: ")?;
    if api_key.is_empty() {
        return Err(anyhow!("API key cannot be empty"));
    }

    let default_model = prompt_input("  Enter default model (for sonnet/opus/small_fast): ")?;
    let haiku_model = prompt_input("  Enter haiku model (optional, press Enter to skip): ")?;

    let env = EnvSettings {
        anthropic_base_url: Some(base_url),
        anthropic_auth_token: Some(api_key),
        api_timeout_ms: Some("3000000".to_string()),
        claude_code_disable_nonessential_traffic: Some(1),
        anthropic_model: Some(default_model.clone()).filter(|s| !s.is_empty()),
        anthropic_small_fast_model: Some(default_model.clone()).filter(|s| !s.is_empty()),
        anthropic_default_sonnet_model: Some(default_model.clone()).filter(|s| !s.is_empty()),
        anthropic_default_opus_model: Some(default_model.clone()).filter(|s| !s.is_empty()),
        anthropic_default_haiku_model: Some(haiku_model).filter(|s| !s.is_empty()),
    };

    let settings = ClaudeSettings {
        env,
        enabled_plugins: Value::Object(serde_json::Map::new()),
        always_thinking_enabled: None,
        other: Value::Object(serde_json::Map::new()),
    };

    let providers_dir = ensure_providers_dir()?;
    let provider_path = providers_dir.join(format!("{}.json", name));

    let content = serde_json::to_string_pretty(&settings)?;
    fs::write(&provider_path, content)?;

    append_provider_function(&name)?;

    println!();
    println!("  ✓ Provider '{}' saved to {}", name, provider_path.display());
    println!("  ✓ Shell functions created for bash and zsh: '{}'", name);
    println!();
    print!("  Press Enter to continue...");
    io::stdout().flush().unwrap();
    let _ = io::stdin().read_line(&mut String::new());

    Ok(())
}

fn remove_provider_interactive() -> Result<()> {
    let providers = list_providers()?;

    if providers.is_empty() {
        clear_screen();
        let mut stdout = io::stdout();
        execute!(stdout, MoveTo(0, 0)).unwrap();
        print!("\n  No providers configured.\n");
        wait_for_key();
        return Ok(());
    }

    let options: Vec<&str> = providers.iter().map(|s| s.as_str()).collect();
    let idx = draw_menu_with_arrows(&options, "Remove Provider");

    if idx >= providers.len() {
        return Ok(());
    }

    let provider_name = &providers[idx];
    let providers_dir = ensure_providers_dir()?;
    let provider_path = providers_dir.join(format!("{}.json", provider_name));

    fs::remove_file(&provider_path)?;

    remove_provider_function(provider_name)?;

    clear_screen();
    let mut stdout = io::stdout();
    execute!(stdout, MoveTo(0, 0)).unwrap();
    print!("\n  Provider '{}' removed.\n", provider_name);
    wait_for_key();

    Ok(())
}

fn build_env_object(env: &EnvSettings) -> serde_json::Map<String, Value> {
    let mut obj = serde_json::Map::new();

    if let Some(v) = &env.anthropic_base_url {
        obj.insert("ANTHROPIC_BASE_URL".to_string(), Value::String(v.clone()));
    }
    if let Some(v) = &env.anthropic_auth_token {
        obj.insert("ANTHROPIC_AUTH_TOKEN".to_string(), Value::String(v.clone()));
    }
    if let Some(v) = &env.api_timeout_ms {
        obj.insert("API_TIMEOUT_MS".to_string(), Value::String(v.clone()));
    }
    if let Some(v) = env.claude_code_disable_nonessential_traffic {
        obj.insert("CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC".to_string(), Value::Number(v.into()));
    }
    if let Some(v) = &env.anthropic_model {
        obj.insert("ANTHROPIC_MODEL".to_string(), Value::String(v.clone()));
    }
    if let Some(v) = &env.anthropic_small_fast_model {
        obj.insert("ANTHROPIC_SMALL_FAST_MODEL".to_string(), Value::String(v.clone()));
    }
    if let Some(v) = &env.anthropic_default_sonnet_model {
        obj.insert("ANTHROPIC_DEFAULT_SONNET_MODEL".to_string(), Value::String(v.clone()));
    }
    if let Some(v) = &env.anthropic_default_opus_model {
        obj.insert("ANTHROPIC_DEFAULT_OPUS_MODEL".to_string(), Value::String(v.clone()));
    }
    if let Some(v) = &env.anthropic_default_haiku_model {
        obj.insert("ANTHROPIC_DEFAULT_HAIKU_MODEL".to_string(), Value::String(v.clone()));
    }

    obj
}

fn run_with_provider(provider_name: &str, args: &[String]) -> Result<()> {
    let providers_dir = ensure_providers_dir()?;
    let provider_path = providers_dir.join(format!("{}.json", provider_name));

    if !provider_path.exists() {
        return Err(anyhow!("Provider '{}' not found. Run 'claude-provider setup' first.", provider_name));
    }

    let provider_content = fs::read_to_string(&provider_path)?;
    let provider_settings: ClaudeSettings = serde_json::from_str(&provider_content)?;

    let config_dir = get_config_dir()?;
    let settings_path = config_dir.join(SETTINGS_FILE);
    let settings_content = fs::read_to_string(&settings_path)?;
    let mut settings: Value = serde_json::from_str(&settings_content)?;

    let env_obj = build_env_object(&provider_settings.env);
    settings.as_object_mut().expect("settings should be an object").insert("env".to_string(), Value::Object(env_obj));

    let modified_content = serde_json::to_string_pretty(&settings)?;
    fs::write(&settings_path, modified_content)?;

    disable_raw_mode().ok();

    let status = Command::new("claude")
        .args(args)
        .status()
        .context("Failed to execute claude")?;

    fs::write(&settings_path, settings_content)?;

    enable_raw_mode().ok();

    if !status.success() {
        return Err(anyhow!("claude exited with non-zero status"));
    }

    Ok(())
}

fn list_providers_command() -> Result<()> {
    let providers = list_providers()?;

    println!();
    if providers.is_empty() {
        println!("  No providers configured.");
    } else {
        println!("  Configured providers:");
        println!();
        for provider in &providers {
            println!("    {}  (type '{}' to launch)", provider, provider);
        }
    }
    println!();

    Ok(())
}

fn detect_shell_command() -> Result<()> {
    let shell = detect_shell();
    println!();
    match shell {
        Shell::Bash => println!("  Detected shell: bash"),
        Shell::Zsh => println!("  Detected shell: zsh"),
    }
    println!();
    Ok(())
}

#[derive(Parser, Debug)]
#[command(name = "claude-provider")]
#[command(author = "User")]
#[command(version = "0.1.0")]
#[command(about = "Manage Claude Code providers and run with custom configs", long_about = None)]
struct Args {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    Setup,

    Remove,

    List,

    Detect,

    Use {
        provider: String,

        #[arg(trailing_var_arg = true)]
        args: Vec<String>,
    },

    Interactive,
}

fn main() -> Result<()> {
    let args = Args::parse();

    match args.command {
        Commands::Setup => {
            setup_provider_interactive()?;
        }
        Commands::Remove => {
            remove_provider_interactive()?;
        }
        Commands::List => {
            list_providers_command()?;
        }
        Commands::Detect => {
            detect_shell_command()?;
        }
        Commands::Use { provider, args } => {
            enable_raw_mode().context("Failed to enable raw mode")?;
            let result = run_with_provider(&provider, &args);
            disable_raw_mode().ok();
            result?;
        }
        Commands::Interactive => {
            clear_screen();
            let mut _raw_guard = match RawModeGuard::new() {
                Ok(g) => g,
                Err(e) => {
                    eprintln!("Failed to enable raw mode: {:#}", e);
                    return Err(e);
                }
            };

            loop {
                let options = vec![
                    "Setup a new provider",
                    "Remove a provider",
                    "List providers",
                    "Exit",
                ];
                let choice = draw_menu_with_arrows(&options, "Claude Provider Manager");

                match choice {
                    0 => {
                        drop(_raw_guard);
                        if let Err(e) = setup_provider_interactive() {
                            eprintln!("Error: {:#}", e);
                            wait_for_key();
                        }
                        _raw_guard = match RawModeGuard::new() {
                            Ok(g) => g,
                            Err(e) => {
                                eprintln!("Failed to restore raw mode: {:#}", e);
                                return Err(e);
                            }
                        };
                    }
                    1 => {
                        drop(_raw_guard);
                        if let Err(e) = remove_provider_interactive() {
                            eprintln!("Error: {:#}", e);
                            wait_for_key();
                        }
                        _raw_guard = match RawModeGuard::new() {
                            Ok(g) => g,
                            Err(e) => {
                                eprintln!("Failed to restore raw mode: {:#}", e);
                                return Err(e);
                            }
                        };
                    }
                    2 => {
                        drop(_raw_guard);
                        let _ = list_providers_command();
                        wait_for_key();
                        _raw_guard = match RawModeGuard::new() {
                            Ok(g) => g,
                            Err(e) => {
                                eprintln!("Failed to restore raw mode: {:#}", e);
                                return Err(e);
                            }
                        };
                    }
                    3 | _ => {
                        drop(_raw_guard);
                        clear_screen();
                        break;
                    }
                }
            }
        }
    }

    Ok(())
}
